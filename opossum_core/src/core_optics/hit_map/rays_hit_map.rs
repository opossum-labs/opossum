//! The structures for storing the actual hitmap.
//!
//! This module also contains the routines for generating a fluence map using different estimator strategies.
use crate::{
    error::{OpmResult, OpossumError},
    joule,
    kde::Kde,
    light::{RayMesh, RaySource, ray_mesh::RayVertex},
    micrometer,
    nodes::fluence_detector::{fluence_data::FluenceData, fluence_grid::FluenceGrid},
    types::{SiScalarSchema, schema_helper_types::Point3Schema},
    utils::try_f64_to_usize,
};
use libm::modf;
use nalgebra::{DMatrix, Point2, Point3};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{ops::Range, sync::Arc};
use uom::si::f64::{Energy, Length};

use super::{
    fluence_estimator::FluenceEstimator,
    ray_tubes::{Arrival, ArrivedSheet, draw_deviation, draw_map},
};

/// Where a ray hit a surface and the energy it brought there.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct HitPoint {
    /// Where the ray hit, in the coordinates of the surface.
    #[schemars(with = "Point3Schema")]
    position: Point3<Length>,
    /// The energy the ray brought.
    #[schemars(with = "SiScalarSchema")]
    energy: Energy,
}
impl HitPoint {
    /// Creates a new [`HitPoint`].
    ///
    /// # Errors
    /// This function returns an error if the energy is negative or not finite, or if the position
    /// is not finite.
    pub fn new(position: Point3<Length>, energy: Energy) -> OpmResult<Self> {
        if !energy.is_finite() || energy.is_sign_negative() {
            return Err(OpossumError::Other(
                "the energy of a hit must be positive and finite".into(),
            ));
        }
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(OpossumError::Other(
                "the position of a hit must be finite".into(),
            ));
        }
        Ok(Self { position, energy })
    }
    /// Returns where the ray hit, in the coordinates of the surface.
    #[must_use]
    pub fn position(&self) -> Point3<Length> {
        self.position
    }
    /// Returns the energy the ray brought.
    #[must_use]
    pub const fn energy(&self) -> Energy {
        self.energy
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
///Storage struct for hitpoints on a surface from a single ray bundle
pub struct RaysHitMap {
    hit_points: Vec<HitPoint>,
    /// The ray tubes the hits stand in, in the order they first hit (see [`RayMesh`]). Not
    /// written to file.
    #[serde(skip)]
    meshes: Vec<Arc<RayMesh>>,
    /// For each hit, the slot of its mesh in `meshes` and the index of its ray there. It holds an
    /// entry for every hit or none at all, the latter as soon as a hit stands in no ray tubes.
    #[serde(skip)]
    vertices: Vec<(u32, u32)>,
}
impl RaysHitMap {
    /// Creates a new [`RaysHitMap`] of the given hits, which follow no ray tubes.
    #[must_use]
    pub const fn new(hit_points: Vec<HitPoint>) -> Self {
        Self {
            hit_points,
            meshes: Vec::new(),
            vertices: Vec::new(),
        }
    }
    /// Add intersection point (with its energy) to this [`RaysHitMap`].
    ///
    /// # Arguments
    ///
    /// * `hit_point` - where the ray hit and what it brought.
    /// * `source` - where the ray stands in the ray tubes of its source, if it does. The map
    ///   follows the tubes only while every one of its hits stands in them.
    ///
    /// # Errors
    /// This function errors if the hits stand in more than `u32::MAX` meshes.
    pub fn add_hit_point(
        &mut self,
        hit_point: HitPoint,
        source: Option<&RaySource>,
    ) -> OpmResult<()> {
        let in_tubes = self.follows_tubes();
        self.hit_points.push(hit_point);
        match source {
            Some(source) if in_tubes => {
                let slot = self
                    .meshes
                    .iter()
                    .position(|mesh| Arc::ptr_eq(mesh, source.mesh()))
                    .unwrap_or_else(|| {
                        self.meshes.push(Arc::clone(source.mesh()));
                        self.meshes.len() - 1
                    });
                let slot = u32::try_from(slot).map_err(|_| {
                    OpossumError::Analysis("too many ray meshes on one surface".into())
                })?;
                self.vertices.push((slot, source.index()));
            }
            _ => {
                self.meshes.clear();
                self.vertices.clear();
            }
        }
        Ok(())
    }
    /// Returns whether every hit of this map stands in the ray tubes of its source.
    const fn follows_tubes(&self) -> bool {
        self.vertices.len() == self.hit_points.len()
    }
    /// Merge this [`RaysHitMap`] with another [`RaysHitMap`].
    ///
    /// The merged map follows no ray tubes: it is made for the estimators that need none.
    pub fn merge(&mut self, other_map: &Self) {
        self.meshes.clear();
        self.vertices.clear();
        self.hit_points.extend_from_slice(&other_map.hit_points);
    }
    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Binning" method
    ///
    /// Each hit spreads its energy over the four points of the map around it, the nearer a point
    /// the more. Each point shows the energy it got over the area it stands for (see
    /// [`FluenceGrid::point_areas`]), so the map holds the energy of the hits within it. Hits
    /// beyond the given ranges are left out.
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range`: optional range along x the map covers, from the first to the last point; that of the hits if `None`
    /// -`ax_2_range`: optional range along y the map covers, from the first to the last point; that of the hits if `None`
    ///
    /// # Errors
    /// This function errors if no ranges are given and the [`RaysHitMap`] is empty.
    pub fn calc_fluence_with_binning(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        let (x, y) = if let (Some(x), Some(y)) = (ax_1_range, ax_2_range) {
            (x.clone(), y.clone())
        } else {
            let min_margin = micrometer!(10.0);
            let (mut left, mut right, mut top, mut bottom) =
                self.calc_2d_bounding_box(micrometer!(0.0))?;
            if (left - right).abs() < min_margin || (top - bottom).abs() < min_margin {
                (left, right, top, bottom) = self.calc_2d_bounding_box(min_margin)?;
            }
            (left..right, bottom..top)
        };
        let grid = FluenceGrid::new(&x, &y, nr_of_points);
        let mut energies = DMatrix::from_element(nr_of_points.1, nr_of_points.0, joule!(0.0));
        for hit in &self.hit_points {
            let Some((along_x, along_y)) = grid.position_of(hit.position.xy()) else {
                continue;
            };
            let ((fx, column), (fy, row)) = (modf(along_x), modf(along_y));
            let (Some(column), Some(row)) = (try_f64_to_usize(column), try_f64_to_usize(row))
            else {
                continue;
            };
            for (next_row, next_column, share) in [
                (0, 0, (1.0 - fx) * (1.0 - fy)),
                (0, 1, fx * (1.0 - fy)),
                (1, 0, (1.0 - fx) * fy),
                (1, 1, fx * fy),
            ] {
                if share > 0.0
                    && let Some(energy) = energies.get_mut((row + next_row, column + next_column))
                {
                    *energy += hit.energy * share;
                }
            }
        }
        let fluences = energies.zip_map(&grid.point_areas(), |energy, area| energy / area);
        Ok(FluenceData::new(fluences, x, y, FluenceEstimator::Binning))
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] that follows the ray tubes
    /// its hits stand in (see [`RayMesh`]), as the Voronoi estimator does.
    ///
    /// A corner shows the energy its rays brought, spread over the area the corner stands for at
    /// its source and magnified as much as its tubes grew on the way. Between the corners the map
    /// runs linearly. Only the tubes whose corners all arrived here are drawn, so
    /// the map stays dark where light was cut away, as in the hole behind a bored mirror.
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range`: optional range of the axis 1 the map covers, all the tubes draw if `None`
    /// -`ax_2_range`: optional range of the axis 2 the map covers, all the tubes draw if `None`
    /// -`estimator`: the estimator the map stands for
    ///
    /// # Errors
    /// This function errors if the hits do not all stand in the ray tubes of their sources, or if
    /// no tube arrived complete.
    pub fn calc_fluence_with_tubes(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
        estimator: FluenceEstimator,
    ) -> OpmResult<FluenceData> {
        draw_map(
            &self.arrived_sheets()?,
            nr_of_points,
            ax_1_range,
            ax_2_range,
            estimator,
        )
    }
    /// Returns the given map of the hits of this [`RaysHitMap`], drawn with
    /// [`RaysHitMap::calc_fluence_with_tubes`], with the estimated deviation of the fluence from it
    /// at each of its points (see [`FluenceData::with_deviation`] and the handbook,
    /// "Fluence uncertainty"). The estimate fits a curvature at every corner, so a map has it only
    /// when asked.
    ///
    /// # Errors
    /// This function errors if the hits do not all stand in the ray tubes of their sources.
    pub fn estimate_deviation(&self, map: FluenceData) -> OpmResult<FluenceData> {
        Ok(draw_deviation(&self.arrived_sheets()?, map))
    }
    /// Returns the sheets of ray tubes the hits of this map stand in, each with the corners that
    /// arrived here and the fluence they show.
    ///
    /// A corner arrived if all its rays hit here. It arrived where its first ray hit and brought
    /// the energy of all its rays together. A ray that hits again, as both arms of an
    /// interferometer do where they meet, arrives in another copy of its sheet: the k-th hit of
    /// each ray belongs to the k-th copy.
    ///
    /// # Errors
    ///
    /// This function returns an error if the hits do not all stand in ray tubes.
    pub(super) fn arrived_sheets(&self) -> OpmResult<Vec<ArrivedSheet<'_>>> {
        // where the hit with the given index lies on the surface, in meters, and its energy
        let hit_of = |hit: usize| {
            self.hit_points.get(hit).map(|hit| {
                (
                    Point2::new(hit.position.x.value, hit.position.y.value),
                    hit.energy,
                )
            })
        };
        if !self.follows_tubes() {
            return Err(OpossumError::Analysis(
                "the rays on this surface span no ray tubes, which a Voronoi map follows".into(),
            ));
        }
        // per mesh: how often each ray hit so far, and the copies of its sheets they arrived in
        let mut tables = self
            .meshes
            .iter()
            .map(|mesh| {
                (
                    vec![0_usize; mesh.nr_of_rays()],
                    Vec::<Vec<Option<usize>>>::new(),
                )
            })
            .collect::<Vec<_>>();
        for (hit, (slot, ray)) in self.vertices.iter().enumerate() {
            let (Some((counts, copies)), ray) = (tables.get_mut(*slot as usize), *ray as usize)
            else {
                continue;
            };
            let nr_of_rays = counts.len();
            let Some(count) = counts.get_mut(ray) else {
                continue;
            };
            if copies.len() <= *count {
                copies.push(vec![None; nr_of_rays]);
            }
            if let Some(entry) = copies.get_mut(*count).and_then(|copy| copy.get_mut(ray)) {
                *entry = Some(hit);
            }
            *count += 1;
        }
        let mut sheets = Vec::new();
        for (mesh, (_, copies)) in self.meshes.iter().zip(&tables) {
            for hit_of_ray in copies {
                let arrival_of = |corner: &RayVertex| {
                    let mut hits = corner
                        .rays()
                        .iter()
                        .map(|ray| hit_of_ray.get(*ray as usize).copied().flatten());
                    let (position, mut energy) = hit_of(hits.next()??)?;
                    for hit in hits {
                        energy += hit_of(hit?)?.1;
                    }
                    Some(Arrival { position, energy })
                };
                sheets.extend(
                    mesh.sheets()
                        .iter()
                        .map(|sheet| ArrivedSheet::new(sheet, arrival_of)),
                );
            }
        }
        Ok(sheets)
    }
    fn calc_2d_bounding_box(&self, margin: Length) -> OpmResult<(Length, Length, Length, Length)> {
        if !margin.is_finite() {
            return Err(OpossumError::Other("margin must be finite".into()));
        }
        self.hit_points.first().map_or_else(
            || {
                Err(OpossumError::Other(
                    "could not calculate bounding box".into(),
                ))
            },
            |hit_point| {
                let mut left = hit_point.position.x;
                let mut right = hit_point.position.x;
                let mut top = hit_point.position.y;
                let mut bottom = hit_point.position.y;
                for point in &self.hit_points {
                    if point.position.x < left {
                        left = point.position.x;
                    }
                    if point.position.y < bottom {
                        bottom = point.position.y;
                    }
                    if point.position.x > right {
                        right = point.position.x;
                    }
                    if point.position.y > top {
                        top = point.position.y;
                    }
                }
                left -= margin;
                right += margin;
                bottom -= margin;
                top += margin;
                Ok((left, right, top, bottom))
            },
        )
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Kernel Density Estimator (KDE)" method
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated    
    ///
    /// # Errors
    /// This function errors if no bandwidth for the kernel can be estimated.
    pub fn calc_fluence_with_kde(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        let mut kde = Kde::default();
        let accessor = |p: &HitPoint| (p.position.xy(), p.energy);
        let pos_accessor = |p: &HitPoint| p.position.xy();
        let mut est_bandwidth = Kde::bandwidth_estimate(&self.hit_points, pos_accessor);
        if !est_bandwidth.is_normal() {
            est_bandwidth = micrometer!(10.0);
        }
        kde.set_band_width(est_bandwidth)?;
        let (left, right, top, bottom) =
            if let (Some(range_1), Some(range_2)) = (ax_1_range, ax_2_range) {
                (range_1.start, range_1.end, range_2.start, range_2.end)
            } else {
                self.calc_2d_bounding_box(3. * est_bandwidth)?
            };
        let fluence_matrix = kde.kde_2d(
            &self.hit_points,
            accessor,
            &(left..right, bottom..top),
            nr_of_points,
        );
        Ok(FluenceData::new(
            fluence_matrix,
            left..right,
            bottom..top,
            FluenceEstimator::KDE,
        ))
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`].
    ///
    /// Create a fluence map with the given number of points and the concrete estimator algorithm.
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`estimator`: Reference to the [`FluenceEstimator`] that should be used toestimate the fluence value.
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated
    ///
    /// # Errors
    /// This function will return an error if the underlying concrete estimator function returns an error.
    pub fn calc_fluence_map(
        &self,
        nr_of_points: (usize, usize),
        estimator: &FluenceEstimator,
        ax_1_range_opt: Option<&Range<Length>>,
        ax_2_range_opt: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        match estimator {
            FluenceEstimator::Voronoi => self.calc_fluence_with_tubes(
                nr_of_points,
                ax_1_range_opt,
                ax_2_range_opt,
                *estimator,
            ),
            FluenceEstimator::KDE => {
                self.calc_fluence_with_kde(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
            FluenceEstimator::Binning => {
                self.calc_fluence_with_binning(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
        }
    }

    /// Returns the hits of this [`RaysHitMap`], in the order they hit.
    #[must_use]
    pub fn hit_points(&self) -> &[HitPoint] {
        &self.hit_points
    }
}
#[cfg(test)]
mod test_hitpoint {
    use crate::{core_optics::hit_map::rays_hit_map::HitPoint, error::OpmResult, joule, meter};
    #[test]
    fn new() {
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::NAN)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::INFINITY)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(-0.1)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::NEG_INFINITY)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(0.0)).is_ok());

        assert!(HitPoint::new(meter!(f64::NAN, 1.0, 1.0), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(f64::INFINITY, 1.0, 1.0), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(f64::NEG_INFINITY, 1.0, 1.0), joule!(1.0)).is_err());

        assert!(HitPoint::new(meter!(1.0, f64::NAN, 1.0), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(1.0, f64::INFINITY, 1.0), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(1.0, f64::NEG_INFINITY, 1.0), joule!(1.0)).is_err());

        assert!(HitPoint::new(meter!(1.0, 1.0, f64::NAN), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, f64::INFINITY), joule!(1.0)).is_err());
        assert!(HitPoint::new(meter!(1.0, 1.0, f64::NEG_INFINITY), joule!(1.0)).is_err());
    }
    #[test]
    fn getters() -> OpmResult<()> {
        let hm = HitPoint::new(meter!(1.0, 2.0, 3.0), joule!(4.0))?;
        assert_eq!(hm.position().x, meter!(1.0));
        assert_eq!(hm.position().y, meter!(2.0));
        assert_eq!(hm.position().z, meter!(3.0));
        assert_eq!(hm.energy(), joule!(4.0));
        Ok(())
    }
}
#[cfg(test)]
mod test_rays_hit_map {
    use super::RaysHitMap;
    use crate::{
        centimeter,
        core_optics::hit_map::{
            HitMap, fluence_estimator::FluenceEstimator, rays_hit_map::HitPoint,
        },
        distributions::position::Grid,
        error::OpmResult,
        joule,
        light::{Ray, Rays},
        meter, nanometer,
    };
    use approx::assert_relative_eq;
    use nalgebra::Point2;
    use spade::Triangulation;
    use uom::si::{length::centimeter, radiant_exposure::joule_per_square_centimeter};
    use uuid::Uuid;

    /// Bins hits of the given energies in J at the given points in m on a grid of the given
    /// number of points over the given ranges in m, or over the hits if `None`.
    fn binned(
        hits: &[((f64, f64), f64)],
        ranges: Option<(std::ops::Range<f64>, std::ops::Range<f64>)>,
        nr_of_points: (usize, usize),
    ) -> OpmResult<crate::nodes::fluence_detector::fluence_data::FluenceData> {
        let mut map = RaysHitMap::default();
        for ((x, y), energy) in hits {
            map.add_hit_point(HitPoint::new(meter!(*x, *y, 0.0), joule!(*energy))?, None)?;
        }
        let ranges = ranges.map(|(x, y)| {
            (
                meter!(x.start)..meter!(x.end),
                meter!(y.start)..meter!(y.end),
            )
        });
        map.calc_fluence_with_binning(
            nr_of_points,
            ranges.as_ref().map(|(x, _)| x),
            ranges.as_ref().map(|(_, y)| y),
        )
    }
    /// The values of the given map in J/m², a row per point along y from its lower end.
    fn values_of(map: &crate::nodes::fluence_detector::fluence_data::FluenceData) -> Vec<Vec<f64>> {
        map.interp_distribution()
            .row_iter()
            .map(|row| row.iter().map(|fluence| fluence.value).collect())
            .collect()
    }
    /// Binned over given ranges, a hit lands where it lies, with the upper half of the range in
    /// the upper rows, and the map shows its energy over the area of the point it lands on.
    #[test]
    fn binning_over_given_ranges_is_upright_and_positive() -> OpmResult<()> {
        // points 1 m apart from 0 m to 4 m: the hit at (1 m, 3 m) lands on row 3, column 1
        let map = binned(&[((1.0, 3.0), 1.0)], Some((0.0..4.0, 0.0..4.0)), (5, 5))?;
        let values = values_of(&map);
        assert_eq!(values[3][1], 1.0);
        assert_eq!(values.iter().flatten().filter(|v| **v != 0.0).count(), 1);
        assert_eq!(map.y_range(), &(meter!(0.0)..meter!(4.0)));
        Ok(())
    }
    /// Hits beyond the given ranges are left out of the map, which holds the others.
    #[test]
    fn binning_leaves_out_hits_beyond_the_given_ranges() -> OpmResult<()> {
        let hits = [((5.0, 1.0), 1.0), ((1.0, -0.5), 1.0), ((1.0, 1.0), 1.0)];
        let map = binned(&hits, Some((0.0..4.0, 0.0..4.0)), (5, 5))?;
        assert_relative_eq!(map.total_energy().value, 1.0);
        Ok(())
    }
    /// A hit in the middle of every cell spreads evenly: each point of the map stands for the
    /// part of the cells around it within the map, half of one on an edge, a quarter in a
    /// corner, and shows the same fluence.
    #[test]
    fn a_hit_in_the_middle_of_each_cell_bins_evenly() -> OpmResult<()> {
        let hits = (0..4)
            .flat_map(|i| (0..4).map(move |j| ((f64::from(i) + 0.5, f64::from(j) + 0.5), 1.0)))
            .collect::<Vec<_>>();
        let map = binned(&hits, Some((0.0..4.0, 0.0..4.0)), (5, 5))?;
        for value in values_of(&map).iter().flatten() {
            assert_relative_eq!(*value, 1.0, max_relative = 1e-12);
        }
        Ok(())
    }
    /// A binned map holds the energy of its hits, wherever they lie.
    #[test]
    fn binning_keeps_the_energy_of_its_hits() -> OpmResult<()> {
        let hits = [
            ((0.0, 0.0), 1.0),
            ((0.3, 1.7), 2.0),
            ((2.0, 0.4), 0.5),
            ((1.1, 1.1), 0.25),
            ((2.0, 2.0), 1.0),
        ];
        let map = binned(&hits, None, (7, 5))?;
        assert_relative_eq!(map.total_energy().value, 4.75, max_relative = 1e-12);
        Ok(())
    }
    /// A KDE map shows a hit where its axes put it: at the point the axes place on the hit, the
    /// map shows the peak of the kernel, `E / (2π σ²)`, here with the 10 µm a single hit gets.
    #[test]
    fn a_kde_map_shows_a_hit_where_its_axes_put_it() -> OpmResult<()> {
        let hits = RaysHitMap::new(vec![HitPoint::new(meter!(0.003, 0.001, 0.0), joule!(1.0))?]);
        let map = hits.calc_fluence_with_kde(
            (5, 3),
            Some(&(meter!(0.0)..meter!(0.004))),
            Some(&(meter!(0.0)..meter!(0.002))),
        )?;
        let (xs, ys) = map.grid().points();
        let on = |at: f64| move |point: &uom::si::f64::Length| (point.value - at).abs() < 1e-12;
        let column = xs
            .iter()
            .position(on(0.003))
            .expect("a point at the hit along x");
        let row = ys
            .iter()
            .position(on(0.001))
            .expect("a point at the hit along y");
        let sigma = 10e-6;
        assert_relative_eq!(
            map.interp_distribution()[(row, column)].value,
            1.0 / (2.0 * std::f64::consts::PI * sigma * sigma),
            max_relative = 1e-9
        );
        Ok(())
    }
    /// A grid of `n_x` × `n_y` rays 1 cm apart around the axis, spanning their ray tubes.
    fn ray_grid(n_x: usize, n_y: usize) -> OpmResult<Rays> {
        let mut rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Grid::new(
                centimeter!((n_x - 1) as f64, (n_y - 1) as f64),
                Point2::new(n_x, n_y),
            )?,
        )?;
        rays.attach_mesh()?;
        Ok(rays)
    }
    /// The area the given ray stands for at its source, in cm² (see
    /// [`RayVertex::cell_area`](crate::light::ray_mesh::RayVertex::cell_area)).
    fn cell_area_of(ray: &Ray) -> f64 {
        ray.source()
            .and_then(|source| {
                source
                    .mesh()
                    .sheets()
                    .iter()
                    .flat_map(|sheet| sheet.vertices())
                    .find(|corner| corner.data().rays().contains(&source.index()))
                    .map(|corner| corner.data().cell_area() * 1e4)
            })
            .unwrap_or_default()
    }
    /// The hit of the given ray where it starts, with the energy that shows 10 J/cm² on the axis,
    /// 1 J/cm² more per cm along x and 3 J/cm² more per cm along y over the area the ray stands
    /// for.
    fn linear_hit(ray: &Ray) -> OpmResult<HitPoint> {
        hit_showing(ray, linear_fluence)
    }
    /// 10 J/cm² on the axis, 1 J/cm² more per cm along x and 3 J/cm² more per cm along y, at the
    /// given point in cm.
    fn linear_fluence(x: f64, y: f64) -> f64 {
        10.0 + x + 3.0 * y
    }
    /// The hit of the given ray where it starts, with the energy that shows the given fluence in
    /// J/cm², of the point in cm, over the area the ray stands for.
    fn hit_showing(ray: &Ray, fluence: impl Fn(f64, f64) -> f64) -> OpmResult<HitPoint> {
        let p = ray.position();
        let value = fluence(p.x.get::<centimeter>(), p.y.get::<centimeter>());
        HitPoint::new(p, joule!(value * cell_area_of(ray)))
    }
    /// A map of the hits of [`linear_hit`] of [`ray_grid`].
    fn linear_hit_map(n_x: usize, n_y: usize) -> OpmResult<RaysHitMap> {
        hit_map_showing(n_x, n_y, linear_fluence)
    }
    /// A map of the hits of [`ray_grid`] that show the given fluence (see [`hit_showing`]).
    fn hit_map_showing(
        n_x: usize,
        n_y: usize,
        fluence: impl Fn(f64, f64) -> f64,
    ) -> OpmResult<RaysHitMap> {
        let mut map = RaysHitMap::default();
        for ray in ray_grid(n_x, n_y)?.iter() {
            map.add_hit_point(hit_showing(ray, &fluence)?, ray.source())?;
        }
        Ok(map)
    }
    /// The map of the given hits on 41 × 41 points over the square of ±2 cm, with the map, the
    /// deviation it estimates and the given fluence, all in J/cm², at each point. On a grid of
    /// 7 × 7 rays 1 cm apart, every corner of the tubes there has neighbors all around.
    fn deviation_check(
        hits: &RaysHitMap,
        fluence: impl Fn(f64, f64) -> f64,
    ) -> OpmResult<Vec<(f64, f64, f64)>> {
        let range = centimeter!(-2.0)..centimeter!(2.0);
        let map = hits.estimate_deviation(hits.calc_fluence_with_tubes(
            (41, 41),
            Some(&range),
            Some(&range),
            FluenceEstimator::Voronoi,
        )?)?;
        let deviation = map
            .deviation()
            .ok_or_else(|| crate::error::OpossumError::Other("no deviation".into()))?;
        let (xs, ys, values) = map.get_fluence_distribution();
        let mut points = Vec::new();
        for (row, y) in ys.iter().enumerate() {
            for (column, x) in xs.iter().enumerate() {
                points.push((
                    values[(row, column)].get::<joule_per_square_centimeter>(),
                    deviation[(row, column)].get::<joule_per_square_centimeter>(),
                    fluence(x.get::<centimeter>(), y.get::<centimeter>()),
                ));
            }
        }
        Ok(points)
    }
    /// A map estimates the deviation of the fluence from it only when asked, since the estimate
    /// fits a curvature at every corner.
    #[test]
    fn a_map_estimates_no_deviation_unless_asked() -> OpmResult<()> {
        let hits = linear_hit_map(4, 4)?;
        let map = hits.calc_fluence_with_tubes((11, 11), None, None, FluenceEstimator::Voronoi)?;
        assert!(map.deviation().is_none());
        assert!(hits.estimate_deviation(map)?.deviation().is_some());
        Ok(())
    }
    /// For a quadratic fluence the estimated deviation is exact: the map plus it shows the fluence
    /// itself, between the rays too (see the handbook, "Fluence uncertainty").
    #[test]
    fn the_deviation_of_a_quadratic_fluence_is_exact() -> OpmResult<()> {
        let quadratic = |x: f64, y: f64| 0.5f64.mul_add(x * y, 50.0 - 2.0 * x * x - y * y);
        for (map, deviation, fluence) in
            deviation_check(&hit_map_showing(7, 7, quadratic)?, quadratic)?
        {
            assert_relative_eq!(map + deviation, fluence, max_relative = 1e-9);
        }
        Ok(())
    }
    /// A linear fluence deviates nowhere from its map.
    #[test]
    fn a_linear_fluence_deviates_nowhere() -> OpmResult<()> {
        let linear = |x: f64, y: f64| 20.0 + x + 3.0 * y;
        for (_, deviation, fluence) in deviation_check(&hit_map_showing(7, 7, linear)?, linear)? {
            assert!(deviation.abs() < 1e-9 * fluence, "{deviation} J/cm²");
        }
        Ok(())
    }
    /// An even fluence deviates nowhere from its map.
    #[test]
    fn a_flat_top_deviates_nowhere() -> OpmResult<()> {
        let even = |_: f64, _: f64| 10.0;
        for (_, deviation, _) in deviation_check(&hit_map_showing(7, 7, even)?, even)? {
            assert!(deviation.abs() < 1e-9, "{deviation} J/cm²");
        }
        Ok(())
    }
    #[test]
    fn new() -> OpmResult<()> {
        let rhm = RaysHitMap::new(vec![]);
        assert_eq!(rhm.hit_points.len(), 0);
        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?;
        let rhm = RaysHitMap::new(vec![hp]);
        assert_eq!(rhm.hit_points.len(), 1);
        Ok(())
    }
    #[test]
    fn add_hit_point() -> OpmResult<()> {
        let mut rhm = RaysHitMap::default();
        assert_eq!(rhm.hit_points.len(), 0);
        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?;
        rhm.add_hit_point(hp, None)?;
        assert_eq!(rhm.hit_points.len(), 1);
        Ok(())
    }
    #[test]
    fn merge() -> OpmResult<()> {
        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?;
        let mut rhm = RaysHitMap::new(vec![hp]);
        let hp2 = HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(1.0))?;
        let rhm2 = RaysHitMap::new(vec![hp2]);
        rhm.merge(&rhm2);
        assert_eq!(rhm.hit_points.len(), 2);
        Ok(())
    }
    #[test]
    fn calc_2d_bounding_box() -> OpmResult<()> {
        let mut rhm = RaysHitMap::default();
        assert!(rhm.calc_2d_bounding_box(meter!(0.0)).is_err());

        rhm.add_hit_point(HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?, None)?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0))?,
            (meter!(0.0), meter!(0.0), meter!(0.0), meter!(0.0))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0))?, None)?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0))?,
            (meter!(-1.0), meter!(0.0), meter!(1.0), meter!(0.0))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0))?, None)?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5))?,
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-0.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, -1.0, 0.0), joule!(1.0))?, None)?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5))?,
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-1.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 2.0, 0.0), joule!(1.0))?, None)?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5))?,
            (meter!(-1.5), meter!(0.5), meter!(2.5), meter!(-1.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(1.0, 2.0, 0.0), joule!(1.0))?, None)?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5))?,
            (meter!(-1.5), meter!(1.5), meter!(2.5), meter!(-1.5))
        );
        assert!(rhm.calc_2d_bounding_box(meter!(f64::NAN)).is_err());
        assert!(rhm.calc_2d_bounding_box(meter!(f64::INFINITY)).is_err());
        assert!(rhm.calc_2d_bounding_box(meter!(f64::NEG_INFINITY)).is_err());
        Ok(())
    }

    #[test]
    fn fluence_matrix_has_expected_dimensions() -> OpmResult<()> {
        let nr_of_points = (5, 4);
        let result = linear_hit_map(4, 3)?.calc_fluence_with_tubes(
            nr_of_points,
            None,
            None,
            FluenceEstimator::Voronoi,
        )?;
        let matrix = result.interp_distribution();
        assert_eq!(matrix.ncols(), nr_of_points.0);
        assert_eq!(matrix.nrows(), nr_of_points.1);
        Ok(())
    }
    /// Two rays span no tube, so they leave no map.
    #[test]
    fn too_few_points_should_fail() -> OpmResult<()> {
        let error = linear_hit_map(2, 1)?
            .calc_fluence_with_tubes((4, 4), None, None, FluenceEstimator::Voronoi)
            .expect_err("no tube");
        assert!(error.to_string().contains("no ray tube"), "{error}");
        Ok(())
    }
    #[test]
    fn fluence_values_are_positive_and_finite() -> OpmResult<()> {
        let res = linear_hit_map(4, 4)?.calc_fluence_with_tubes(
            (6, 5),
            None,
            None,
            FluenceEstimator::Voronoi,
        )?;
        for (idx, val) in res.interp_distribution().iter().enumerate() {
            assert!(
                val.is_finite() && val.value >= 0.0,
                "Value at Index {} is invalid: {}",
                idx,
                val.value
            );
        }
        Ok(())
    }
    #[test]
    fn combined_fluence_respects_dimensions() -> OpmResult<()> {
        let mut scene = HitMap::default();
        let id = Uuid::new_v4();
        for ray in ray_grid(3, 3)?.iter() {
            scene.add_to_hitmap(linear_hit(ray)?, 0, id, ray.source())?;
        }
        let nr_of_points = (10, 6);
        let fluence_data =
            scene.calc_combined_fluence_with_tubes(nr_of_points, FluenceEstimator::Voronoi)?;
        let mat = fluence_data.interp_distribution();
        assert_eq!(mat.ncols(), nr_of_points.0);
        assert_eq!(mat.nrows(), nr_of_points.1);
        Ok(())
    }
    /// Between its corners a tube map runs linearly, so it shows a linear fluence as it is.
    #[test]
    fn values_increase_consistently_with_position() -> OpmResult<()> {
        let range = centimeter!(-1.0)..centimeter!(1.0);
        let res = linear_hit_map(3, 3)?.calc_fluence_with_tubes(
            (5, 5),
            Some(&range),
            Some(&range),
            FluenceEstimator::Voronoi,
        )?;
        let at = |row, column| {
            res.interp_distribution()[(row, column)].get::<joule_per_square_centimeter>()
        };
        assert_relative_eq!(at(1, 1), 8.0, max_relative = 1e-9);
        assert_relative_eq!(at(3, 3), 12.0, max_relative = 1e-9);
        assert_relative_eq!(at(1, 3), 9.0, max_relative = 1e-9);
        Ok(())
    }
}
#[cfg(test)]
mod test_ray_tubes {
    use super::RaysHitMap;
    use crate::{
        core_optics::hit_map::{fluence_estimator::FluenceEstimator, rays_hit_map::HitPoint},
        distributions::position::Grid,
        error::OpmResult,
        joule,
        light::{Ray, Rays},
        millimeter, nanometer,
        nodes::fluence_detector::fluence_data::FluenceData,
        utils::test_helper::helper::{assert_warned, check_logs, fluence_near, hexapolar_bundle},
    };
    use approx::assert_relative_eq;
    use nalgebra::{Point2, Point3};
    use uom::si::{
        energy::joule, f64::Length, length::millimeter,
        radiant_exposure::joule_per_square_centimeter,
    };

    /// The bundle of [`hexapolar_bundle`], 5 mm in radius, spanning its ray tubes.
    fn bundle_in_tubes(rings: u8) -> OpmResult<Rays> {
        let mut rays = hexapolar_bundle(rings)?;
        rays.attach_mesh()?;
        Ok(rays)
    }
    /// The fluence in J/cm² of [`bundle_in_tubes`].
    fn flat_top_fluence(rings: u8) -> f64 {
        crate::utils::test_helper::helper::flat_top_fluence(5.0, rings)
    }
    /// Whether the given ray starts closer to the axis than `radius` (in mm).
    fn starts_within(ray: &Ray, radius: f64) -> bool {
        let p = ray.position();
        p.x.get::<millimeter>().hypot(p.y.get::<millimeter>()) < radius
    }
    /// Adds the hits of the given rays to the given map, each where `place` puts it, leaving out
    /// the rays it puts nowhere.
    fn add_placed_hits(
        map: &mut RaysHitMap,
        rays: &Rays,
        place: impl Fn(&Ray) -> Option<Point3<Length>>,
    ) -> OpmResult<()> {
        for ray in rays.iter() {
            if let Some(at) = place(ray) {
                map.add_hit_point(HitPoint::new(at, ray.energy())?, ray.source())?;
            }
        }
        Ok(())
    }
    /// Adds the hits of the given rays where they start, `scale` times as far from the axis, to
    /// the given map, leaving out the rays that start closer to the axis than `hole` (in mm).
    fn add_hits(map: &mut RaysHitMap, rays: &Rays, scale: f64, hole: f64) -> OpmResult<()> {
        add_placed_hits(map, rays, |ray| {
            let p = ray.position();
            (!starts_within(ray, hole)).then(|| Point3::new(p.x * scale, p.y * scale, p.z))
        })
    }
    /// A map of the hits of the given rays where they start, with x moved to where `fold` puts
    /// it, given x and y.
    fn hits_folded(rays: &Rays, fold: impl Fn(Length, Length) -> Length) -> OpmResult<RaysHitMap> {
        let mut map = RaysHitMap::default();
        add_placed_hits(&mut map, rays, |ray| {
            let p = ray.position();
            Some(Point3::new(fold(p.x, p.y), p.y, p.z))
        })?;
        Ok(map)
    }
    /// A map of the hits of [`add_hits`].
    fn hits_of(rays: &Rays, scale: f64, hole: f64) -> OpmResult<RaysHitMap> {
        let mut map = RaysHitMap::default();
        add_hits(&mut map, rays, scale, hole)?;
        Ok(map)
    }
    /// The Voronoi map of the given hits on a grid of 121 × 121 points `half_width` mm around the
    /// axis.
    fn map_of(hits: &RaysHitMap, half_width: f64) -> OpmResult<FluenceData> {
        let range = millimeter!(-half_width)..millimeter!(half_width);
        hits.calc_fluence_with_tubes(
            (121, 121),
            Some(&range),
            Some(&range),
            FluenceEstimator::Voronoi,
        )
    }

    /// A corner needs five neighbors around it to tell the curvature of the fluence: those of the
    /// outermost ring of a hexapolar bundle have fewer and tell none, all others tell one.
    #[test]
    fn a_corner_with_too_few_neighbors_has_no_curvature() -> OpmResult<()> {
        let rays = bundle_in_tubes(3)?;
        let hits = hits_of(&rays, 1.0, 0.0)?;
        let sheets = hits.arrived_sheets()?;
        let [sheet] = sheets.as_slice() else {
            panic!("one sheet");
        };
        let curvatures = sheet.curvatures();
        let outermost = rays.iter().filter(|ray| !starts_within(ray, 4.9)).count();
        assert_eq!(outermost, 18);
        assert_eq!(curvatures.iter().filter(|c| c.is_none()).count(), outermost);
        Ok(())
    }
    /// Hits of rays that span no tubes leave the map without them, and a Voronoi map says it
    /// needs them.
    #[test]
    fn hits_without_tubes_leave_no_voronoi_map() -> OpmResult<()> {
        let map = hits_of(&hexapolar_bundle(3)?, 1.0, 0.0)?;
        assert!(map.vertices.is_empty());
        let error = map_of(&map, 6.0).expect_err("no tubes");
        assert!(error.to_string().contains("span no ray tubes"), "{error}");
        Ok(())
    }
    /// A map without hits shows no Voronoi map: no tube arrived to draw.
    #[test]
    fn a_map_without_hits_shows_no_voronoi_map() {
        let map = RaysHitMap::default();
        assert!(
            map.calc_fluence_with_tubes((11, 11), None, None, FluenceEstimator::Voronoi)
                .is_err()
        );
    }
    /// A bundle that arrives twice, as both arms of a Mach-Zehnder interferometer do at its
    /// output, adds up: the second arrival of each ray is a second copy of its sheet.
    #[test]
    fn a_bundle_arriving_twice_adds_up() -> OpmResult<()> {
        let rays = bundle_in_tubes(10)?;
        let mut map = hits_of(&rays, 1.0, 0.0)?;
        add_hits(&mut map, &rays, 1.0, 0.0)?;
        let map = map_of(&map, 6.0)?;
        assert_relative_eq!(
            fluence_near(&map, 2.0, 1.0)?,
            2.0 * flat_top_fluence(10),
            max_relative = 0.03
        );
        assert_relative_eq!(map.total_energy().value, 2.0, max_relative = 0.03);
        Ok(())
    }
    /// Without a given range a map reaches as far as the light it draws: the margins beyond the
    /// outermost rays included, so it holds all of the energy.
    #[test]
    fn a_map_reaches_as_far_as_its_light() -> OpmResult<()> {
        let hits = hits_of(&bundle_in_tubes(10)?, 1.0, 0.0)?;
        let map =
            hits.calc_fluence_with_tubes((121, 121), None, None, FluenceEstimator::Voronoi)?;
        assert!(
            map.x_range().end.get::<millimeter>() > 5.2,
            "{:?}",
            map.x_range()
        );
        assert!(
            map.y_range().start.get::<millimeter>() < -5.2,
            "{:?}",
            map.y_range()
        );
        assert_relative_eq!(map.total_energy().value, 1.0, max_relative = 0.03);
        Ok(())
    }
    /// One hit outside the tubes ends them for the whole map.
    #[test]
    fn a_hit_outside_the_tubes_ends_them() -> OpmResult<()> {
        let mut map = hits_of(&bundle_in_tubes(3)?, 1.0, 0.0)?;
        assert!(map.follows_tubes());
        map.add_hit_point(
            HitPoint::new(millimeter!(0.0, 0.0, 0.0), joule!(1.0))?,
            None,
        )?;
        assert!(!map.follows_tubes());
        assert!(map.vertices.is_empty() && map.meshes.is_empty());
        Ok(())
    }
    /// Hits of two meshes keep them apart, and a merged map follows no tubes.
    #[test]
    fn hits_of_two_meshes_keep_them_apart() -> OpmResult<()> {
        let (first, second) = (bundle_in_tubes(1)?, bundle_in_tubes(1)?);
        let mut map = hits_of(&first, 1.0, 0.0)?;
        add_hits(&mut map, &second, 1.0, 0.0)?;
        assert_eq!(map.meshes.len(), 2);
        let slots = map
            .vertices
            .iter()
            .map(|(slot, _)| *slot)
            .collect::<Vec<_>>();
        assert_eq!(slots, [[0; 7], [1; 7]].concat());
        map.merge(&hits_of(&first, 1.0, 0.0)?);
        assert!(!map.follows_tubes());
        Ok(())
    }
    /// A flat-top beam shows its fluence all over the map, which holds all of its energy. Its
    /// center is left out: the ray there stands for less area than the others, and the same
    /// energy shows there as a fifth more fluence.
    #[test]
    fn a_flat_top_shows_its_fluence_and_holds_its_energy() -> OpmResult<()> {
        let map = map_of(&hits_of(&bundle_in_tubes(10)?, 1.0, 0.0)?, 6.0)?;
        for (x, y) in [(2.0, 1.0), (-3.0, 2.5), (1.0, -4.0), (0.0, 3.5)] {
            assert_relative_eq!(
                fluence_near(&map, x, y)?,
                flat_top_fluence(10),
                max_relative = 0.03
            );
        }
        assert_relative_eq!(map.total_energy().value, 1.0, max_relative = 0.03);
        Ok(())
    }
    /// A beam twice as wide spreads its energy over four times the area.
    #[test]
    fn a_magnified_beam_shows_a_quarter_of_the_fluence() -> OpmResult<()> {
        let map = map_of(&hits_of(&bundle_in_tubes(10)?, 2.0, 0.0)?, 12.0)?;
        for (x, y) in [(4.0, 2.0), (-6.0, 5.0), (0.0, -7.0)] {
            assert_relative_eq!(
                fluence_near(&map, x, y)?,
                flat_top_fluence(10) / 4.0,
                max_relative = 0.03
            );
        }
        assert_relative_eq!(map.total_energy().value, 1.0, max_relative = 0.03);
        Ok(())
    }
    /// Light cut away in the middle leaves the map dark there, and right next to the cut it shows
    /// the full fluence: the map holds the energy that is left.
    #[test]
    fn a_hole_in_the_beam_stays_dark() -> OpmResult<()> {
        let rays = bundle_in_tubes(10)?;
        let hits = hits_of(&rays, 1.0, 2.2)?;
        let map = map_of(&hits, 6.0)?;
        assert!(fluence_near(&map, 0.0, 0.0)?.abs() < 1e-12);
        assert!(fluence_near(&map, 1.5, 0.0)?.abs() < 1e-12);
        assert_relative_eq!(
            fluence_near(&map, 2.75, 0.0)?,
            flat_top_fluence(10),
            max_relative = 0.05
        );
        assert_relative_eq!(
            fluence_near(&map, 0.0, -2.75)?,
            flat_top_fluence(10),
            max_relative = 0.05
        );
        let left = rays
            .iter()
            .filter(|ray| !starts_within(ray, 2.2))
            .map(|ray| ray.energy().get::<joule>())
            .sum::<f64>();
        assert_relative_eq!(map.total_energy().value, left, max_relative = 0.03);
        Ok(())
    }
    /// The rays of a square grid show an even fluence, however the squares between them are cut
    /// into tubes.
    #[test]
    fn a_square_grid_shows_an_even_fluence() -> OpmResult<()> {
        let mut rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Grid::new(millimeter!(10.0, 10.0), Point2::new(11, 11))?,
        )?;
        rays.attach_mesh()?;
        let hits = hits_of(&rays, 1.0, 0.0)?;
        // the points of the map lie on the rays, 1 mm apart
        let range = millimeter!(-5.0)..millimeter!(5.0);
        let map = hits.calc_fluence_with_tubes(
            (11, 11),
            Some(&range),
            Some(&range),
            FluenceEstimator::Voronoi,
        )?;
        let per_ray = 1.0 / 121.0 / 0.01;
        for row in 1..10 {
            for column in 1..10 {
                assert_relative_eq!(
                    map.interp_distribution()[(row, column)].get::<joule_per_square_centimeter>(),
                    per_ray,
                    max_relative = 1e-9
                );
            }
        }
        Ok(())
    }
    /// Light folded over itself, as at a caustic, lies twice where it overlaps, and the map
    /// says that it folds: its left half turned over onto its right half shows twice the fluence
    /// there and none on the left.
    #[test]
    fn light_folded_over_itself_adds_up_and_warns() -> OpmResult<()> {
        testing_logger::setup();
        let map = map_of(&hits_folded(&bundle_in_tubes(10)?, |x, _| x.abs())?, 6.0)?;
        assert_warned("folds over itself");
        for (x, y) in [(2.0, 1.0), (3.0, -2.5)] {
            assert_relative_eq!(
                fluence_near(&map, x, y)?,
                2.0 * flat_top_fluence(10),
                max_relative = 0.03
            );
            assert!(fluence_near(&map, -x, y)?.abs() < 1e-12);
        }
        assert_relative_eq!(map.total_energy().value, 1.0, max_relative = 0.03);
        Ok(())
    }
    /// Light turned over as a whole, as behind a focus or a mirror, does not fold.
    #[test]
    fn light_turned_over_as_a_whole_does_not_fold() -> OpmResult<()> {
        testing_logger::setup();
        let map = map_of(&hits_folded(&bundle_in_tubes(10)?, |x, _| -x)?, 6.0)?;
        check_logs(log::Level::Warn, vec![]);
        assert_relative_eq!(
            fluence_near(&map, 2.0, 1.0)?,
            flat_top_fluence(10),
            max_relative = 0.03
        );
        Ok(())
    }
    /// The margins at the edges of a flat-top beam hold as much of its energy as of its area:
    /// each reaches half a ring spacing `d` beyond its edge. A beam of radius `R` keeps
    /// `(R d + d²/4) / (R + d/2)²` of its energy there, a ring from `r` to `R` the share
    /// `d / (R − r + d)`.
    #[test]
    fn the_margins_hold_their_share_of_the_energy() -> OpmResult<()> {
        let rays = bundle_in_tubes(10)?;
        let (d, outer): (f64, f64) = (0.5, 5.0);
        let beam = map_of(&hits_of(&rays, 1.0, 0.0)?, 6.0)?;
        let share = outer.mul_add(d, d * d / 4.0) / (outer + d / 2.0).powi(2);
        assert_relative_eq!(
            beam.edge_share().unwrap_or(f64::NAN),
            share,
            max_relative = 0.02
        );
        // the rings 0.5 mm apart that are left reach from 2.5 mm to 5 mm
        let ring = map_of(&hits_of(&rays, 1.0, 2.2)?, 6.0)?;
        assert_relative_eq!(
            ring.edge_share().unwrap_or(f64::NAN),
            d / (outer - 2.5 + d),
            max_relative = 0.02
        );
        Ok(())
    }
    /// The margins at the edge of a hole meet on the bisectors of its corners instead of
    /// overlapping: however finely the map looks, it nowhere shows more than the flat top.
    #[test]
    fn the_margins_at_a_hole_do_not_overlap() -> OpmResult<()> {
        // rings 1 mm apart: the edge of the hole runs through 18 rays, 20° apart
        let hits = hits_of(&bundle_in_tubes(5)?, 1.0, 2.2)?;
        let range = millimeter!(-3.5)..millimeter!(3.5);
        let map = hits.calc_fluence_with_tubes(
            (401, 401),
            Some(&range),
            Some(&range),
            FluenceEstimator::Voronoi,
        )?;
        let peak = map.peak().get::<joule_per_square_centimeter>();
        assert!(
            peak < 1.1 * flat_top_fluence(5),
            "the map shows {peak} J/cm² against {} J/cm²",
            flat_top_fluence(5)
        );
        Ok(())
    }
    /// A square beam is even right into its corners, where the margins of its sides meet.
    #[test]
    fn a_square_beam_is_even_into_its_corners() -> OpmResult<()> {
        // 5 × 5 rays 1 mm apart carry 1 J over 5 mm × 5 mm, the margins included: 4 J/cm²
        let mut rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Grid::new(millimeter!(4.0, 4.0), Point2::new(5, 5))?,
        )?;
        rays.attach_mesh()?;
        let map = map_of(&hits_of(&rays, 1.0, 0.0)?, 2.6)?;
        for (x, y) in [(2.0, 2.0), (2.35, 2.25), (-2.4, 2.2), (0.0, 2.4)] {
            assert_relative_eq!(fluence_near(&map, x, y)?, 4.0, max_relative = 1e-6);
        }
        Ok(())
    }
    /// A map that does not follow ray tubes has no margins whose share it could tell.
    #[test]
    fn a_kde_map_tells_no_share_of_margins() -> OpmResult<()> {
        let map =
            hits_of(&bundle_in_tubes(3)?, 1.0, 0.0)?.calc_fluence_with_kde((21, 21), None, None)?;
        assert!(map.edge_share().is_none());
        Ok(())
    }
    /// Light squashed to a line, as at a line focus, does not count as folded, even where
    /// rounding turns some of its tubes over: here its upper half turns over and its lower half
    /// does not, both squashed to a trillionth.
    #[test]
    fn light_squashed_to_a_line_does_not_fold() -> OpmResult<()> {
        testing_logger::setup();
        let squash = |x: Length, y: Length| x * 1e-12 * y.value.signum();
        map_of(&hits_folded(&bundle_in_tubes(10)?, squash)?, 6.0)?;
        check_logs(log::Level::Warn, vec![]);
        Ok(())
    }
}
