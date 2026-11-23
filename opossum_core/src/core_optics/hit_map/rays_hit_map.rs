//! The structures for storing the actual hitmap.
//!
//! This module also conatins the routines for genearating a fluence map using different estimator strategies.
use crate::{
    J_per_cm2, centimeter,
    error::{OpmResult, OpossumError},
    kde::Kde,
    light::{RayMesh, RaySource, ray_mesh::RayVertex},
    meter, micrometer,
    nodes::fluence_detector::{Fluence, fluence_data::FluenceData},
    utils::{
        AxLims,
        griddata::{
            VoronoiedData, create_voronoi_cells, grid_interpolate_3d_triangulated_scatter_data,
            linspace,
        },
        to_f64, try_f64_to_usize,
    },
};
use itertools::Itertools;
use libm::modf;
use log::warn;
use nalgebra::{DMatrix, DVector, MatrixXx2, Point2, Point3};
use serde::{Deserialize, Serialize};
use std::{ops::Range, sync::Arc};
use uom::si::{
    energy::joule,
    f64::{Area, Energy, Length},
    length,
    radiant_exposure::joule_per_square_centimeter,
};

use super::{
    fluence_estimator::FluenceEstimator,
    ray_tubes::{Arrival, ArrivedSheet, draw_map},
};

/// Trait to ensure that a generic physical quantity can be validated
/// for being finite and positive.
pub trait QuantityValidation {
    /// Returns true if the underlying value is finite.
    fn is_finite(&self) -> bool;
    /// Returns true if the underlying value is negative.
    fn is_sign_negative(&self) -> bool;
}

// Implement the validation trait for Energy
impl QuantityValidation for Energy {
    fn is_finite(&self) -> bool {
        self.value.is_finite()
    }
    fn is_sign_negative(&self) -> bool {
        self.value.is_sign_negative()
    }
}

// Implement the validation trait for Fluence
impl QuantityValidation for Fluence {
    fn is_finite(&self) -> bool {
        self.value.is_finite()
    }
    fn is_sign_negative(&self) -> bool {
        self.value.is_sign_negative()
    }
}

/// A generic hit point storing a position and a generic value (e.g., Energy or Fluence).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaseHitPoint<V> {
    /// Position of the intersection point
    pub position: Point3<Length>,
    /// Value (Energy or Fluence) of the ray that intersected the surface
    pub value: V,
}

impl<V> BaseHitPoint<V>
where
    V: Copy + QuantityValidation, // Use our custom trait bound here
{
    /// Creates a new generic hit point.
    ///
    /// # Errors
    /// Returns an error if the value is negative or not finite, or if the
    /// position coordinates are not finite.
    pub fn new(position: Point3<Length>, value: V) -> OpmResult<Self> {
        // Correct check for NaN, infinity, and negative values
        if !value.is_finite() || value.is_sign_negative() {
            return Err(OpossumError::Other(
                "Value must be positive and finite".into(),
            ));
        }
        // Validate the position coordinates
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(OpossumError::Other("Position must be finite".into()));
        }
        Ok(Self { position, value })
    }
    /// Returns the position of this hit point.
    #[must_use]
    pub fn position(&self) -> Point3<Length> {
        self.position
    }
    /// Returns the value of this hit point.
    #[must_use]
    pub const fn value(&self) -> V {
        self.value
    }
}

// Define the type aliases for seamless integration
pub type EnergyHitPoint = BaseHitPoint<Energy>;
pub type FluenceHitPoint = BaseHitPoint<Fluence>;

/// Enum to store different types of hit point quantities in a hitmap
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HitPoints {
    ///Energy hitpoints contain a vec of [`EnergyHitPoint`]
    Energy(Vec<EnergyHitPoint>),
    ///Fluence hitpoints contain a vec of [`FluenceHitPoint`]
    Fluence(Vec<FluenceHitPoint>),
}

impl HitPoints {
    /// Returns the length of the store hit-point vector within this [`HitPoints`]
    #[must_use]
    pub const fn len(&self) -> usize {
        match self {
            Self::Energy(vec) => vec.len(),
            Self::Fluence(vec) => vec.len(),
        }
    }
    /// Checks if the stored hit-point vectors are emtpy. Returns true if empty, false otherwise.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        match self {
            Self::Energy(vec) => vec.is_empty(),
            Self::Fluence(vec) => vec.is_empty(),
        }
    }

    /// Returns a vector of 3d positions of the stored hit-point vectors
    #[must_use]
    pub fn positions(&self) -> Vec<Point3<Length>> {
        match self {
            Self::Energy(vec) => vec.iter().map(EnergyHitPoint::position).collect_vec(),
            Self::Fluence(vec) => vec.iter().map(FluenceHitPoint::position).collect_vec(),
        }
    }
}
/// Enum to pass different types of hit point quantities in a hitmap
pub enum HitPoint {
    ///Energy hitpoint contains an [`EnergyHitPoint`]
    Energy(EnergyHitPoint),
    ///Fluence hitpoint contains a [`FluenceHitPoint`]
    Fluence(FluenceHitPoint),
}
impl Default for HitPoints {
    fn default() -> Self {
        Self::Energy(Vec::<EnergyHitPoint>::new())
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
///Storage struct for hitpoints on a surface from a single ray bundle
pub struct RaysHitMap {
    hit_points: HitPoints,
    x_lims: (Length, Length),
    y_lims: (Length, Length),
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
    /// Creates a new [`RaysHitMap`]
    #[must_use]
    pub fn new(hit_points: HitPoints) -> Self {
        let (xmin, xmax, ymin, ymax) = match &hit_points {
            HitPoints::Energy(vec) => vec.iter().fold(
                (
                    meter!(f64::INFINITY),
                    meter!(f64::NEG_INFINITY),
                    meter!(f64::INFINITY),
                    meter!(f64::NEG_INFINITY),
                ),
                |arg0, v| {
                    (
                        arg0.0.min(v.position.x),
                        arg0.1.max(v.position.x),
                        arg0.2.min(v.position.y),
                        arg0.3.max(v.position.y),
                    )
                },
            ),
            HitPoints::Fluence(vec) => vec.iter().fold(
                (
                    meter!(f64::INFINITY),
                    meter!(f64::NEG_INFINITY),
                    meter!(f64::INFINITY),
                    meter!(f64::NEG_INFINITY),
                ),
                |arg0, v| {
                    (
                        arg0.0.min(v.position.x),
                        arg0.1.max(v.position.x),
                        arg0.2.min(v.position.y),
                        arg0.3.max(v.position.y),
                    )
                },
            ),
        };
        Self {
            hit_points,
            x_lims: (xmin, xmax),
            y_lims: (ymin, ymax),
            meshes: Vec::new(),
            vertices: Vec::new(),
        }
    }
    /// Add intersection point (with energy/fluence) to this [`RaysHitMap`].
    ///
    /// # Arguments
    ///
    /// * `hit_point` - where the ray hit and what it brought.
    /// * `source` - where the ray stands in the ray tubes of its source, if it does. The map
    ///   follows the tubes only while every one of its hits stands in them.
    ///
    /// # Errors
    /// This function errors if the hitpoint that should be added does not match the already stored
    /// hit point type, or if the hits stand in more than `u32::MAX` meshes.
    pub fn add_hit_point(
        &mut self,
        hit_point: HitPoint,
        source: Option<&RaySource>,
    ) -> OpmResult<()> {
        let in_tubes = self.follows_tubes() || self.hit_points.is_empty();
        self.push_hit_point(hit_point)?;
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
    fn follows_tubes(&self) -> bool {
        !self.hit_points.is_empty() && self.vertices.len() == self.hit_points.len()
    }
    /// Add an intersection point to the hit points of this map and widen its limits.
    fn push_hit_point(&mut self, hit_point: HitPoint) -> OpmResult<()> {
        match hit_point {
            HitPoint::Energy(energy_hit_point) => {
                if let HitPoints::Energy(v) = &mut self.hit_points {
                    // Update limits based on the new point
                    if v.is_empty() {
                        self.x_lims.0 = energy_hit_point.position.x;
                        self.x_lims.1 = energy_hit_point.position.x;
                        self.y_lims.0 = energy_hit_point.position.y;
                        self.y_lims.1 = energy_hit_point.position.y;
                    } else {
                        self.x_lims.0 = energy_hit_point.position.x.min(self.x_lims.0);
                        self.x_lims.1 = energy_hit_point.position.x.max(self.x_lims.1);
                        self.y_lims.0 = energy_hit_point.position.y.min(self.y_lims.0);
                        self.y_lims.1 = energy_hit_point.position.y.max(self.y_lims.1);
                    }
                    // Fast insertion: just push the point without searching for duplicates
                    v.push(energy_hit_point);
                } else {
                    return Err(OpossumError::Analysis(
                        "wrong hit point type for this hitmap! Must be Energyhitpoint!".into(),
                    ));
                }
            }
            HitPoint::Fluence(fluence_hit_point) => {
                if let HitPoints::Fluence(v) = &mut self.hit_points {
                    // Update limits based on the new point
                    if v.is_empty() {
                        self.x_lims.0 = fluence_hit_point.position.x;
                        self.x_lims.1 = fluence_hit_point.position.x;
                        self.y_lims.0 = fluence_hit_point.position.y;
                        self.y_lims.1 = fluence_hit_point.position.y;
                    } else {
                        self.x_lims.0 = fluence_hit_point.position.x.min(self.x_lims.0);
                        self.x_lims.1 = fluence_hit_point.position.x.max(self.x_lims.1);
                        self.y_lims.0 = fluence_hit_point.position.y.min(self.y_lims.0);
                        self.y_lims.1 = fluence_hit_point.position.y.max(self.y_lims.1);
                    }
                    // Fast insertion: just push the point without searching for duplicates
                    v.push(fluence_hit_point);
                } else {
                    return Err(OpossumError::Analysis(
                        "wrong hit point type for this hitmap! Must be Fluencehitpoint!".into(),
                    ));
                }
            }
        }
        Ok(())
    }
    /// Returns the x limit (min, max) of the [`HitPoints`] that are stored in this [`RaysHitMap`]
    #[must_use]
    pub fn x_lims(&self) -> &(Length, Length) {
        &self.x_lims
    }
    /// Returns the y limit (min, max) of the [`HitPoints`] that are stored in this [`RaysHitMap`]
    #[must_use]
    pub fn y_lims(&self) -> &(Length, Length) {
        &self.y_lims
    }

    /// Merge this [`RaysHitMap`] with another [`RaysHitMap`].
    ///
    /// The merged map follows no ray tubes: it is made for the estimators that need none.
    ///
    /// # Errors
    /// This function errors if the hit-point type of the respective [`RaysHitMap`] does not match
    pub fn merge(&mut self, other_map: &Self) -> OpmResult<()> {
        self.meshes.clear();
        self.vertices.clear();
        match &mut self.hit_points {
            HitPoints::Energy(mut_vec) => match &other_map.hit_points {
                HitPoints::Energy(vec) => {
                    for hit_point in vec {
                        mut_vec.push(hit_point.clone());
                    }
                }
                HitPoints::Fluence(_) => {
                    return Err(OpossumError::Analysis(
                        "wrong hit point type for this hitmap! Must be Fluencehitpoint!".into(),
                    ));
                }
            },
            HitPoints::Fluence(mut_vec) => match &other_map.hit_points {
                HitPoints::Fluence(vec) => {
                    for hit_point in vec {
                        mut_vec.push(hit_point.clone());
                    }
                }
                HitPoints::Energy(_) => {
                    return Err(OpossumError::Analysis(
                        "wrong hit point type for this hitmap! Must be Fluencehitpoint!".into(),
                    ));
                }
            },
        }
        Ok(())
    }
    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Binning" method
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated    
    ///
    /// # Errors
    /// This function errors if
    /// - the [`RaysHitMap`] is empty.
    /// - The hit point type is neither energy nor fluence
    pub fn calc_fluence_with_binning(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        if let HitPoints::Energy(hit_points) = &self.hit_points {
            let (left, right, top, bottom) =
                if let (Some(range_1), Some(range_2)) = (ax_1_range, ax_2_range) {
                    (range_1.start, range_1.end, range_2.start, range_2.end)
                } else {
                    let min_margin = micrometer!(10.0);
                    let (left, right, top, bottom) = self.calc_2d_bounding_box(micrometer!(0.0))?;
                    if (left - right).abs() < min_margin || (top - bottom).abs() < min_margin {
                        self.calc_2d_bounding_box(min_margin)?
                    } else {
                        (left, right, top, bottom)
                    }
                };

            let bin_width: Length = (right - left) / to_f64(nr_of_points.0);
            let bin_height: Length = (top - bottom) / to_f64(nr_of_points.1);

            let bin_area: Area = bin_width * bin_height;

            let width_step = (right - left) / (to_f64(nr_of_points.0 - 1));
            let height_step = (top - bottom) / (to_f64(nr_of_points.1 - 1));

            let mut fluence_matrix = DMatrix::<Fluence>::zeros(nr_of_points.1, nr_of_points.0);
            for hit_point in hit_points {
                let (fract_index_x, int_index_x) =
                    modf((hit_point.position.x - left).value / width_step.value);
                let (fract_index_y, int_index_y) =
                    modf((hit_point.position.y - bottom).value / height_step.value);
                let Some(x_index) = try_f64_to_usize(int_index_x) else {
                    return Err(OpossumError::Analysis("cannot calculate x_index!".into()));
                };
                let Some(y_index) = try_f64_to_usize(int_index_y) else {
                    return Err(OpossumError::Analysis("cannot calculate y_index!".into()));
                };

                let fluence = hit_point.value / bin_area;
                let fluence_x = (1.0 - fract_index_x) * (1.0 - fract_index_y) * fluence;
                let fl_next_x = fract_index_x * (1.0 - fract_index_y) * fluence;
                let fl_next_y = (1.0 - fract_index_x) * fract_index_y * fluence;
                let fl_xy_next = fract_index_x * fract_index_y * fluence;

                fluence_matrix[(y_index, x_index)] += fluence_x;
                if x_index < nr_of_points.0 - 1 {
                    fluence_matrix[(y_index, x_index + 1)] += fl_next_x;
                    if y_index < nr_of_points.1 - 1 {
                        fluence_matrix[(y_index + 1, x_index + 1)] += fl_xy_next;
                    }
                }
                if y_index < nr_of_points.1 - 1 {
                    fluence_matrix[(y_index + 1, x_index)] += fl_next_y;
                }
            }
            Ok(FluenceData::new(
                fluence_matrix,
                left..right,
                bottom..top,
                FluenceEstimator::Binning,
            ))
        } else if let HitPoints::Fluence(_) = &self.hit_points {
            warn!(
                "Unexpected type of HitPoints for binning estimator! Changing to helper-ray estimator!"
            );
            self.calc_fluence_with_helper_rays(nr_of_points, ax_1_range, ax_2_range)
        } else {
            Err(OpossumError::Analysis("wrong hit point type for to calculate fluence with binning! Must be an Energyhitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Voronoi" method
    ///
    /// The map follows the ray tubes the hits stand in (see [`RayMesh`]). Each corner shows the
    /// energy of its rays spread over the Voronoi cell it stands for at its source, magnified as
    /// much as its tubes grew on the way; between the corners the map runs linearly. Only the
    /// tubes whose corners all arrived here are drawn, so the map stays dark where light was cut
    /// away, as in the hole behind a bored mirror.
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range`: optional range of the axis 1 the map covers, all the tubes draw if `None`
    /// -`ax_2_range`: optional range of the axis 2 the map covers, all the tubes draw if `None`
    ///
    /// # Errors
    /// This function errors if the hits do not all stand in the ray tubes of their sources.
    pub fn calc_fluence_with_voronoi(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        match &self.hit_points {
            HitPoints::Energy(_) => Ok(draw_map(
                &self.voronoi_sheets()?,
                nr_of_points,
                ax_1_range,
                ax_2_range,
                self.hit_ranges(),
                FluenceEstimator::Voronoi,
            )),
            HitPoints::Fluence(_) => {
                warn!(
                    "Unexpected type of HitPoints for Voronoi estimator! Changing to helper-ray estimator!"
                );
                self.calc_fluence_with_helper_rays(nr_of_points, ax_1_range, ax_2_range)
            }
        }
    }
    /// Returns the sheets of ray tubes the hits of this map stand in, each with the fluence, in
    /// J/m², its corners show from the energies they brought (see
    /// [`ArrivedSheet::fluences_from_energies`]).
    ///
    /// # Errors
    ///
    /// This function returns an error if the hits are fluences rather than energies, or if they
    /// do not all stand in ray tubes.
    pub(super) fn voronoi_sheets(&self) -> OpmResult<Vec<(ArrivedSheet<'_>, Vec<Option<f64>>)>> {
        let HitPoints::Energy(hit_points) = &self.hit_points else {
            return Err(OpossumError::Analysis(
                "a Voronoi map needs the energies of the hits".into(),
            ));
        };
        let energy_of = |hit: usize| {
            hit_points.get(hit).map(|p| {
                let at = Point2::new(p.position.x.value, p.position.y.value);
                (at, p.value.get::<joule>())
            })
        };
        Ok(self
            .arrived_sheets(energy_of)?
            .into_iter()
            .map(|sheet| {
                let fluences = sheet.fluences_from_energies();
                (sheet, fluences)
            })
            .collect())
    }
    /// Returns the ranges along x and y the hits of this map cover.
    pub(super) fn hit_ranges(&self) -> (Range<Length>, Range<Length>) {
        (self.x_lims.0..self.x_lims.1, self.y_lims.0..self.y_lims.1)
    }
    /// Returns the sheets of ray tubes the hits of this map stand in, each with the corners that
    /// arrived here.
    ///
    /// A corner arrived if all its rays hit here. It arrived where its first ray hit and brought
    /// what all its rays brought together. A ray that hits again, as both arms of an
    /// interferometer do where they meet, arrives in another copy of its sheet: the k-th hit of
    /// each ray belongs to the k-th copy.
    ///
    /// # Arguments
    ///
    /// * `hit_of` - where the hit with the given index lies on the surface, in meters, and what
    ///   it brought: its energy or its fluence.
    ///
    /// # Errors
    ///
    /// This function returns an error if the hits do not all stand in ray tubes.
    fn arrived_sheets(
        &self,
        hit_of: impl Fn(usize) -> Option<(Point2<f64>, f64)>,
    ) -> OpmResult<Vec<ArrivedSheet<'_>>> {
        if !self.follows_tubes() {
            return Err(OpossumError::Analysis(
                "the rays on this surface span no ray tubes, which a Voronoi or helper ray map follows".into(),
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
                    let (position, mut value) = hit_of(hits.next()??)?;
                    for hit in hits {
                        value += hit_of(hit?)?.1;
                    }
                    Some(Arrival { position, value })
                };
                sheets.extend(
                    mesh.sheets()
                        .iter()
                        .map(|sheet| ArrivedSheet::new(sheet, &arrival_of)),
                );
            }
        }
        Ok(sheets)
    }
    fn calc_2d_bounding_box(&self, margin: Length) -> OpmResult<(Length, Length, Length, Length)> {
        if let HitPoints::Energy(hit_points) = &self.hit_points {
            if !margin.is_finite() {
                return Err(OpossumError::Other("margin must be finite".into()));
            }
            hit_points.first().map_or_else(
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
                    for point in hit_points {
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
        } else {
            Err(OpossumError::Analysis("wrong hit point type for to calculate 2d bounding box for kde! Must be an Energyhitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Kernel Density Estimator (KDE)" method
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated    
    ///
    /// # Errors
    /// This function errors if
    /// - no bandwidth for the kernel can be estimated
    /// - The hit point type is neither energy nor fluence
    pub fn calc_fluence_with_kde(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        if let HitPoints::Energy(hit_points) = &self.hit_points {
            let mut kde = Kde::default();
            // Define Accessors
            let accessor = |p: &EnergyHitPoint| (p.position.xy(), p.value);
            let pos_accessor = |p: &EnergyHitPoint| p.position.xy();
            // Estimate bandwidth
            let mut est_bandwidth = Kde::bandwidth_estimate(hit_points, pos_accessor);

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
                hit_points,
                accessor,
                &(left..right, bottom..top),
                nr_of_points,
            );

            let fluence_data = FluenceData::new(
                fluence_matrix,
                left..right,
                bottom..top,
                FluenceEstimator::KDE,
            );
            Ok(fluence_data)
        } else if let HitPoints::Fluence(_) = &self.hit_points {
            warn!(
                "Unexpected type of HitPoints for kernel density estimator! Changing to helper-ray estimator!"
            );
            self.calc_fluence_with_helper_rays(nr_of_points, ax_1_range, ax_2_range)
        } else {
            Err(OpossumError::Analysis("Wrong hit point type for to calculate fluence with kde! Must be an EnergyHitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`] using the "Helper Rays" method
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`ax_1_range_opt`: optional range of the axis 1 on which the data should be interpolated
    /// -`ax_2_range_opt`: optional range of the axis 2 on which the data should be interpolated    
    ///
    /// # Errors
    /// This function errors if
    /// - there are too few data points for voronoi cell creation (less than three)
    /// - the voronoi diagram generation fails
    /// - no axes ranges are provided and the data is not sufficient to provide valid ranges
    /// - the interpolation step fails
    pub fn calc_fluence_with_helper_rays(
        &self,
        nr_of_points: (usize, usize),
        ax_1_range: Option<&Range<Length>>,
        ax_2_range: Option<&Range<Length>>,
    ) -> OpmResult<FluenceData> {
        if let HitPoints::Fluence(hit_points) = &self.hit_points {
            let mut pos_in_cm = MatrixXx2::<f64>::zeros(hit_points.len());
            let mut fluence = DVector::<f64>::zeros(hit_points.len());

            if hit_points.len() < 3 {
                return Err(OpossumError::Other(
                    "Too few points (<3) on hitmap to calculate fluence!".into(),
                ));
            }
            for (row, p) in hit_points.iter().enumerate() {
                pos_in_cm[(row, 0)] = p.position.x.get::<length::centimeter>();
                pos_in_cm[(row, 1)] = p.position.y.get::<length::centimeter>();
                fluence[row] = p.value.get::<joule_per_square_centimeter>();
            }

            let (co_ax1, ax_1_range) = if let Some(range) = ax_1_range {
                (
                    linspace(
                        range.start.get::<length::centimeter>(),
                        range.end.get::<length::centimeter>(),
                        nr_of_points.0,
                    )?,
                    range.clone(),
                )
            } else {
                let proj_ax1_lim =
                    AxLims::finite_from_dvector(&pos_in_cm.column(0)).ok_or_else(|| {
                        OpossumError::Other(
                            "cannot construct valid axis limits with non-finite axes bounds!"
                                .into(),
                        )
                    })?;
                (
                    linspace(proj_ax1_lim.min, proj_ax1_lim.max, nr_of_points.0)?,
                    centimeter!(proj_ax1_lim.min)..centimeter!(proj_ax1_lim.max),
                )
            };

            let (co_ax2, ax_2_range) = if let Some(range) = ax_2_range {
                (
                    linspace(
                        range.start.get::<length::centimeter>(),
                        range.end.get::<length::centimeter>(),
                        nr_of_points.1,
                    )?,
                    range.clone(),
                )
            } else {
                let proj_ax2_lim =
                    AxLims::finite_from_dvector(&pos_in_cm.column(1)).ok_or_else(|| {
                        OpossumError::Other(
                            "cannot construct valid axis limits with non-finite axes bounds!"
                                .into(),
                        )
                    })?;
                (
                    linspace(proj_ax2_lim.min, proj_ax2_lim.max, nr_of_points.1)?,
                    centimeter!(proj_ax2_lim.min)..centimeter!(proj_ax2_lim.max),
                )
            };

            let (voronoi, _beam_area) = create_voronoi_cells(&pos_in_cm).map_err(|e| {
                OpossumError::Other(format!(
                    "Voronoi diagram for fluence interpolation could not be created!: {e}"
                ))
            })?;

            let fluence =
                fluence.insert_rows(hit_points.len(), voronoi.sites.len() - hit_points.len(), 0.);

            let voronoi_fluence_scatter =
                VoronoiedData::combine_data_with_voronoi_diagram(voronoi, fluence)?;

            //currently only interpolation. voronoid data for plotting must still be implemented
            let (interp_fluence, _) = grid_interpolate_3d_triangulated_scatter_data(
                &voronoi_fluence_scatter,
                &co_ax1,
                &co_ax2,
            )?;

            Ok(FluenceData::new(
                DMatrix::from_iterator(
                    co_ax2.len(),
                    co_ax1.len(),
                    interp_fluence.iter().map(|val| J_per_cm2!(*val)),
                ),
                ax_1_range,
                ax_2_range,
                FluenceEstimator::HelperRays,
            ))
        } else if let HitPoints::Energy(_) = &self.hit_points {
            warn!(
                "Unexpected type of HitPoints for helper-ray estimator! Changing to voronoi estimator!"
            );
            self.calc_fluence_with_voronoi(nr_of_points, ax_1_range, ax_2_range)
        } else {
            Err(OpossumError::Analysis("Wrong hit point type for to calculate fluence with helper rays! Must be a FluenceHitpoint!".into()))
        }
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
            FluenceEstimator::Voronoi => {
                self.calc_fluence_with_voronoi(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
            FluenceEstimator::KDE => {
                self.calc_fluence_with_kde(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
            FluenceEstimator::Binning => {
                self.calc_fluence_with_binning(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
            FluenceEstimator::HelperRays => {
                self.calc_fluence_with_helper_rays(nr_of_points, ax_1_range_opt, ax_2_range_opt)
            }
        }
    }

    /// Gets the maximum fluence value off this [`RaysHitMap`]
    ///
    /// # Attributes
    /// -`estimator`: Reference to the [`FluenceEstimator`] that should be used toestimate the fluence value.
    ///
    /// # Errors
    /// This function will return an error if
    /// - `calc_fluence_map` returns an error
    /// - if the `HitPoint` type macthes netieher energy or fluence
    pub fn get_max_fluence(&self, estimator: &FluenceEstimator) -> OpmResult<Fluence> {
        match estimator {
            FluenceEstimator::Voronoi | FluenceEstimator::KDE | FluenceEstimator::Binning => {
                Ok(self
                    .calc_fluence_map((101, 101), estimator, None, None)?
                    .peak())
            }
            FluenceEstimator::HelperRays => {
                if let HitPoints::Fluence(hit_points) = &self.hit_points {
                    Ok(hit_points
                        .iter()
                        .fold(J_per_cm2!(0.), |init, val| val.value.max(init)))
                } else if let HitPoints::Energy(_) = &self.hit_points {
                    Ok(self
                        .calc_fluence_map((101, 101), &FluenceEstimator::Voronoi, None, None)?
                        .peak())
                } else {
                    Err(OpossumError::Analysis("Undefined HitPointType! Cannot use get_max_fluence method to retrieve maximum fluence!".into()))
                }
            }
        }
    }

    /// Returns a reference to the hit map of this [`RaysHitMap`].
    #[must_use]
    pub const fn hit_map(&self) -> &HitPoints {
        &self.hit_points
    }
}
#[cfg(test)]
mod test_hitpoint {
    use crate::{
        J_per_cm2,
        core_optics::hit_map::rays_hit_map::{EnergyHitPoint, FluenceHitPoint},
        error::OpmResult,
        joule, meter,
    };
    #[test]
    fn new_fluence_hit_point() {
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(f64::NAN)).is_err());
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(f64::INFINITY)).is_err());
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(-0.1)).is_err());
        assert!(
            FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(f64::NEG_INFINITY)).is_err()
        );
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(0.0)).is_ok());

        assert!(FluenceHitPoint::new(meter!(f64::NAN, 1.0, 1.0), J_per_cm2!(1.0)).is_err());
        assert!(FluenceHitPoint::new(meter!(f64::INFINITY, 1.0, 1.0), J_per_cm2!(1.0)).is_err());
        assert!(
            FluenceHitPoint::new(meter!(f64::NEG_INFINITY, 1.0, 1.0), J_per_cm2!(1.0)).is_err()
        );

        assert!(FluenceHitPoint::new(meter!(1.0, f64::NAN, 1.0), J_per_cm2!(1.0)).is_err());
        assert!(FluenceHitPoint::new(meter!(1.0, f64::INFINITY, 1.0), J_per_cm2!(1.0)).is_err());
        assert!(
            FluenceHitPoint::new(meter!(1.0, f64::NEG_INFINITY, 1.0), J_per_cm2!(1.0)).is_err()
        );

        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, f64::NAN), J_per_cm2!(1.0)).is_err());
        assert!(FluenceHitPoint::new(meter!(1.0, 1.0, f64::INFINITY), J_per_cm2!(1.0)).is_err());
        assert!(
            FluenceHitPoint::new(meter!(1.0, 1.0, f64::NEG_INFINITY), J_per_cm2!(1.0)).is_err()
        );
    }
    #[test]
    fn new_energy_hit_point() {
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::NAN)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::INFINITY)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(-0.1)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(f64::NEG_INFINITY)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(0.0)).is_ok());

        assert!(EnergyHitPoint::new(meter!(f64::NAN, 1.0, 1.0), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(f64::INFINITY, 1.0, 1.0), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(f64::NEG_INFINITY, 1.0, 1.0), joule!(1.0)).is_err());

        assert!(EnergyHitPoint::new(meter!(1.0, f64::NAN, 1.0), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, f64::INFINITY, 1.0), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, f64::NEG_INFINITY, 1.0), joule!(1.0)).is_err());

        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, f64::NAN), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, f64::INFINITY), joule!(1.0)).is_err());
        assert!(EnergyHitPoint::new(meter!(1.0, 1.0, f64::NEG_INFINITY), joule!(1.0)).is_err());
    }
    #[test]
    fn getters_fluence_hit_point() -> OpmResult<()> {
        let hm = FluenceHitPoint::new(meter!(1.0, 2.0, 3.0), J_per_cm2!(4.0))?;
        assert_eq!(hm.position().x, meter!(1.0));
        assert_eq!(hm.position().y, meter!(2.0));
        assert_eq!(hm.position().z, meter!(3.0));
        assert_eq!(hm.value(), J_per_cm2!(4.0));
        Ok(())
    }
    #[test]
    fn getters_energ_hit_point() -> OpmResult<()> {
        let hm = EnergyHitPoint::new(meter!(1.0, 2.0, 3.0), joule!(4.0))?;
        assert_eq!(hm.position().x, meter!(1.0));
        assert_eq!(hm.position().y, meter!(2.0));
        assert_eq!(hm.position().z, meter!(3.0));
        assert_eq!(hm.value(), joule!(4.0));
        Ok(())
    }
}
#[cfg(test)]
mod test_hitpoints {
    use super::HitPoints;
    use crate::{
        J_per_cm2,
        core_optics::hit_map::rays_hit_map::{EnergyHitPoint, FluenceHitPoint},
        error::OpmResult,
        joule, meter,
    };

    #[test]
    fn len() -> OpmResult<()> {
        let hp = HitPoints::Energy(vec![]);
        assert!(hp.is_empty());
        let hp = HitPoints::Fluence(vec![]);
        assert!(hp.is_empty());

        let hp = HitPoints::Energy(vec![EnergyHitPoint::new(
            meter!(1.0, 2.0, 3.0),
            joule!(4.0),
        )?]);
        assert!(!hp.is_empty());
        assert!(hp.len() == 1);
        let hp = HitPoints::Fluence(vec![
            FluenceHitPoint::new(meter!(1.0, 2.0, 3.0), J_per_cm2!(4.0))?,
            FluenceHitPoint::new(meter!(2.0, 3.0, 4.0), J_per_cm2!(5.0))?,
        ]);
        assert!(!hp.is_empty());
        assert!(hp.len() == 2);
        Ok(())
    }
    #[test]
    fn positions() -> OpmResult<()> {
        let hp = HitPoints::Fluence(vec![
            FluenceHitPoint::new(meter!(1.0, 2.0, 3.0), J_per_cm2!(4.0))?,
            FluenceHitPoint::new(meter!(2.0, 3.0, 4.0), J_per_cm2!(5.0))?,
        ]);
        let pos = hp.positions();
        assert_eq!(pos[0].x.value, 1.);
        assert_eq!(pos[0].y.value, 2.);
        assert_eq!(pos[0].z.value, 3.);
        assert_eq!(pos[1].x.value, 2.);
        assert_eq!(pos[1].y.value, 3.);
        assert_eq!(pos[1].z.value, 4.);
        Ok(())
    }
}
#[cfg(test)]
mod test_rays_hit_map {
    use super::RaysHitMap;
    use crate::{
        J_per_cm2, centimeter,
        core_optics::hit_map::{
            HitMap,
            rays_hit_map::{EnergyHitPoint, FluenceHitPoint, HitPoint, HitPoints},
        },
        error::OpmResult,
        joule, meter,
    };
    use uuid::Uuid;

    // helper function
    fn dummy_fluence_hitpoints(n_x: usize, n_y: usize) -> HitPoints {
        let mut pts = Vec::new();
        for i in 0..n_y {
            for j in 0..n_x {
                pts.push(FluenceHitPoint {
                    position: centimeter!(j as f64, i as f64, 0.0),
                    value: J_per_cm2!((i * n_x + j) as f64 + 1.0),
                });
            }
        }
        HitPoints::Fluence(pts)
    }
    #[test]
    fn lims() -> OpmResult<()> {
        let hp = HitPoints::Fluence(vec![
            FluenceHitPoint::new(meter!(1.0, 2.0, 3.0), J_per_cm2!(4.0))?,
            FluenceHitPoint::new(meter!(2.0, 3.0, 4.0), J_per_cm2!(5.0))?,
        ]);
        let rhm = RaysHitMap::new(hp);
        assert_eq!(rhm.x_lims.0.value, 1.0);
        assert_eq!(rhm.x_lims.1.value, 2.0);
        assert_eq!(rhm.y_lims.0.value, 2.0);
        assert_eq!(rhm.y_lims.1.value, 3.0);
        Ok(())
    }
    #[test]
    fn new_energy_hit_point() -> OpmResult<()> {
        let rhm = RaysHitMap::new(HitPoints::Energy(vec![]));
        assert_eq!(rhm.hit_points.len(), 0);

        let hp = EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?;
        let rhm = RaysHitMap::new(HitPoints::Energy(vec![hp]));
        assert_eq!(rhm.hit_points.len(), 1);
        Ok(())
    }
    #[test]
    fn new_fluence_hit_point() -> OpmResult<()> {
        let rhm = RaysHitMap::new(HitPoints::Fluence(vec![]));
        assert_eq!(rhm.hit_points.len(), 0);

        let hp = FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0))?;
        let rhm = RaysHitMap::new(HitPoints::Fluence(vec![hp]));
        assert_eq!(rhm.hit_points.len(), 1);
        Ok(())
    }
    #[test]
    fn add_to_hitmap_energy_hit_point() -> OpmResult<()> {
        let mut rhm = RaysHitMap::default();
        assert_eq!(rhm.hit_points.len(), 0);
        let hp = EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?;
        rhm.add_hit_point(HitPoint::Energy(hp), None)?;
        assert_eq!(rhm.hit_points.len(), 1);
        Ok(())
    }
    #[test]
    fn add_energy_to_hitmap_fluence_hit_point() -> OpmResult<()> {
        let mut rhm = RaysHitMap::default();
        assert_eq!(rhm.hit_points.len(), 0);
        let hp = FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0))?;
        assert!(rhm.add_hit_point(HitPoint::Fluence(hp), None).is_err());
        Ok(())
    }
    #[test]
    fn merge_energy_hit_point() -> OpmResult<()> {
        let hp = EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?;
        let mut rhm = RaysHitMap::new(HitPoints::Energy(vec![hp]));
        let hp2 = EnergyHitPoint::new(meter!(1.0, 1.0, 1.0), joule!(1.0))?;
        let rhm2 = RaysHitMap::new(HitPoints::Energy(vec![hp2]));
        rhm.merge(&rhm2)?;
        assert_eq!(rhm.hit_points.len(), 2);
        Ok(())
    }
    #[test]
    fn merge_fluence_hit_point() -> OpmResult<()> {
        let hp = FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0))?;
        let mut rhm = RaysHitMap::new(HitPoints::Fluence(vec![hp]));
        let hp2 = FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(1.0))?;
        let rhm2 = RaysHitMap::new(HitPoints::Fluence(vec![hp2]));
        rhm.merge(&rhm2)?;
        assert_eq!(rhm.hit_points.len(), 2);
        Ok(())
    }
    #[test]
    fn merge_fluence_to_energy() -> OpmResult<()> {
        let hp = EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?;
        let mut rhm = RaysHitMap::new(HitPoints::Energy(vec![hp]));
        let hp2 = FluenceHitPoint::new(meter!(1.0, 1.0, 1.0), J_per_cm2!(1.0))?;
        let mut rhm2 = RaysHitMap::new(HitPoints::Fluence(vec![hp2]));
        assert!(rhm.merge(&rhm2).is_err());
        assert!(rhm2.merge(&rhm).is_err());
        Ok(())
    }
    #[test]
    fn calc_2d_bounding_box() -> OpmResult<()> {
        let mut rhm = RaysHitMap::default();
        assert!(rhm.calc_2d_bounding_box(meter!(0.0)).is_err());

        rhm.add_hit_point(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?),
            None,
        )?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0))?,
            (meter!(0.0), meter!(0.0), meter!(0.0), meter!(0.0))
        );
        rhm.add_hit_point(
            HitPoint::Energy(EnergyHitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0))?),
            None,
        )?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0))?,
            (meter!(-1.0), meter!(0.0), meter!(1.0), meter!(0.0))
        );
        rhm.add_hit_point(
            HitPoint::Energy(EnergyHitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0))?),
            None,
        )?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5))?,
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-0.5))
        );
        rhm.add_hit_point(
            HitPoint::Energy(EnergyHitPoint::new(meter!(-1.0, -1.0, 0.0), joule!(1.0))?),
            None,
        )?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5))?,
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-1.5))
        );
        rhm.add_hit_point(
            HitPoint::Energy(EnergyHitPoint::new(meter!(-1.0, 2.0, 0.0), joule!(1.0))?),
            None,
        )?;
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5))?,
            (meter!(-1.5), meter!(0.5), meter!(2.5), meter!(-1.5))
        );
        rhm.add_hit_point(
            HitPoint::Energy(EnergyHitPoint::new(meter!(1.0, 2.0, 0.0), joule!(1.0))?),
            None,
        )?;
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
    fn fluence_matrix_has_expected_dimensions() {
        let ray_map = RaysHitMap {
            hit_points: dummy_fluence_hitpoints(4, 3),
            ..Default::default()
        };

        let nr_of_points = (5, 4);
        let result = ray_map
            .calc_fluence_with_helper_rays(nr_of_points, None, None)
            .expect("Fluence calculation should succeed");

        let matrix = result.interp_distribution();
        assert_eq!(
            matrix.ncols(),
            nr_of_points.0,
            "x-axis should define number of rows"
        );
        assert_eq!(
            matrix.nrows(),
            nr_of_points.1,
            "y-axis should define number of columns"
        );
    }

    #[test]
    fn too_few_points_should_fail() {
        let ray_map = RaysHitMap {
            hit_points: HitPoints::Fluence(vec![
                FluenceHitPoint {
                    position: centimeter!(0.0, 0.0, 0.0),
                    value: J_per_cm2!(1.0),
                },
                FluenceHitPoint {
                    position: centimeter!(1.0, 0.0, 0.0),
                    value: J_per_cm2!(1.0),
                },
            ]),
            ..Default::default()
        };

        let result = ray_map.calc_fluence_with_helper_rays((4, 4), None, None);
        assert!(
            result.is_err(),
            "Less than 3 points should result in an error"
        );
    }

    #[test]
    fn fluence_values_are_positive_and_finite() {
        let ray_map = RaysHitMap {
            hit_points: dummy_fluence_hitpoints(4, 4),
            ..Default::default()
        };

        let res = ray_map
            .calc_fluence_with_helper_rays((6, 5), None, None)
            .expect("Fluence calc should succeed");

        let mat = res.interp_distribution();
        for (idx, val) in mat.iter().enumerate() {
            assert!(
                val.is_finite() && val.value >= 0.0,
                "Value at Index {} is invalid: {}",
                idx,
                val.value
            );
        }
    }

    #[test]
    fn combined_fluence_respects_dimensions() {
        let mut scene = HitMap::default();
        let id = Uuid::new_v4();
        match dummy_fluence_hitpoints(3, 3) {
            HitPoints::Energy(_) => (),
            HitPoints::Fluence(fluence_hit_points) => {
                for hp in fluence_hit_points {
                    let _ = scene.add_to_hitmap(HitPoint::Fluence(hp), 0, id, None);
                }
            }
        }

        let nr_of_points = (10, 6);
        let fluence_data = scene
            .calc_combined_fluence_with_helper_rays(nr_of_points)
            .expect("Combined fluence should succeed");

        let mat = fluence_data.interp_distribution();
        assert_eq!(mat.ncols(), nr_of_points.0);
        assert_eq!(mat.nrows(), nr_of_points.1);
    }

    #[test]
    fn values_increase_consistently_with_position() {
        let ray_map = RaysHitMap {
            hit_points: dummy_fluence_hitpoints(3, 3),
            ..Default::default()
        };

        let res = ray_map
            .calc_fluence_with_helper_rays((3, 3), None, None)
            .expect("Fluence calc should succeed");

        let mat = res.interp_distribution();
        assert!(mat[(0, 0)] < mat[(2, 2)]);
    }
}
#[cfg(test)]
mod test_ray_tubes {
    use super::RaysHitMap;
    use crate::{
        core_optics::hit_map::rays_hit_map::{EnergyHitPoint, HitPoint},
        distributions::position::Grid,
        error::OpmResult,
        joule,
        light::{Ray, Rays},
        millimeter, nanometer,
        nodes::fluence_detector::fluence_data::FluenceData,
        utils::test_helper::helper::{energy_of, fluence_near, hexapolar_bundle},
    };
    use approx::assert_relative_eq;
    use nalgebra::{Point2, Point3};
    use uom::si::{
        energy::joule, length::millimeter, radiant_exposure::joule_per_square_centimeter,
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
    /// Adds the hits of the given rays where they start, `scale` times as far from the axis, to
    /// the given map, leaving out the rays that start closer to the axis than `hole` (in mm).
    fn add_hits(map: &mut RaysHitMap, rays: &Rays, scale: f64, hole: f64) -> OpmResult<()> {
        for ray in rays.iter().filter(|ray| !starts_within(ray, hole)) {
            let p = ray.position();
            let at = Point3::new(p.x * scale, p.y * scale, p.z);
            map.add_hit_point(
                HitPoint::Energy(EnergyHitPoint::new(at, ray.energy())?),
                ray.source(),
            )?;
        }
        Ok(())
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
        hits.calc_fluence_with_voronoi((121, 121), Some(&range), Some(&range))
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
        assert_relative_eq!(energy_of(&map), 2.0, max_relative = 0.03);
        Ok(())
    }
    /// Without a given range a map reaches as far as the light it draws: the margins beyond the
    /// outermost rays included, so it holds all of the energy.
    #[test]
    fn a_map_reaches_as_far_as_its_light() -> OpmResult<()> {
        let hits = hits_of(&bundle_in_tubes(10)?, 1.0, 0.0)?;
        let map = hits.calc_fluence_with_voronoi((121, 121), None, None)?;
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
        assert_relative_eq!(energy_of(&map), 1.0, max_relative = 0.03);
        Ok(())
    }
    /// One hit outside the tubes ends them for the whole map.
    #[test]
    fn a_hit_outside_the_tubes_ends_them() -> OpmResult<()> {
        let mut map = hits_of(&bundle_in_tubes(3)?, 1.0, 0.0)?;
        assert!(map.follows_tubes());
        map.add_hit_point(
            HitPoint::Energy(EnergyHitPoint::new(
                millimeter!(0.0, 0.0, 0.0),
                joule!(1.0),
            )?),
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
        map.merge(&hits_of(&first, 1.0, 0.0)?)?;
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
        assert_relative_eq!(energy_of(&map), 1.0, max_relative = 0.03);
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
        assert_relative_eq!(energy_of(&map), 1.0, max_relative = 0.03);
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
        assert_relative_eq!(energy_of(&map), left, max_relative = 0.03);
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
        let map = hits.calc_fluence_with_voronoi((11, 11), Some(&range), Some(&range))?;
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
}
