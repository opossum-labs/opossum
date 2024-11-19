//! Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
//! [`OpticSurface`](crate::surface::optic_surface::OpticSurface).
//!
//! A [`HitMap`] not only stores the hit points but also the number of bounces a [`Ray`](crate::ray::Ray) has
//! undergone before hitting a surface and the [`Uuid`] of the ray bundle that caused the hit.
//!
//! The overall structure is a follows (in ascending hierarchy):
//!
//!  - The most basic structure is a [`HitPoint`] storing a [`Ray`s](crate::ray::Ray) intersection point with
//!    a surface and its energy.
//!  - A [`RaysHitMap`] simply stores a vector of [`HitPoint`]s. It also implements functions for calculating a fluence
//!    map (using either the Voronoi or the KDE method).
//!  - A [`BouncedHitMap`] stores a [`RaysHitMap`] together with an [`Uuid`] of the ray bundle ([`Rays`](crate::rays::Rays)).
//!  - A [`HitMap`] stores a vector of [`BouncedHitMap`]s. The vector index represents the number of ray bounces. So, the
//!    first entry contains all [`BouncedHitMap`]s caused by rays wih zero bounces, the second entry all [`BouncedHitMap`]s
//!    caused by rays wih one bounce, ...
//!  
use crate::{
    centimeter,
    error::{OpmResult, OpossumError},
    kde::Kde,
    nodes::fluence_detector::{fluence_data::FluenceData, Fluence},
    plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::Proptype,
    utils::{
        griddata::{
            calc_closed_poly_area, create_voronoi_cells, interpolate_3d_triangulated_scatter_data,
            linspace, VoronoiedData,
        },
        unit_format::{
            get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
            get_unit_value_as_length_with_format_by_exponent,
        },
    },
    J_per_cm2,
};
use log::warn;
use nalgebra::{DMatrix, DVector, MatrixXx2, Point2, Point3};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length,
};
use uuid::Uuid;

/// Strategy for fluence estimation
#[derive(Serialize, Deserialize, Debug, Clone)]
#[non_exhaustive]
pub enum FluenceEstimator {
    /// Calculate Voronoi cells of the hit points and use the cell area for calculation of the fluence.
    Voronoi,
    /// Calculate the fluence at given point using a Kernel Density Estimator
    KDE,
    /// Simply perform binning of the hit points on a given matrix (not implemented yet)
    Binning,
}
impl Display for FluenceEstimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Voronoi => write!(f, "Voronoi"),
            Self::KDE => write!(f, "KDE"),
            Self::Binning => write!(f, "Binning"),
        }
    }
}
impl From<FluenceEstimator> for Proptype {
    fn from(value: FluenceEstimator) -> Self {
        Self::FluenceEstimator(value)
    }
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
/// Storage struct for `RaysHitMap` on a surface from a single bounce
pub struct BouncedHitMap {
    hit_map: HashMap<Uuid, RaysHitMap>,
}
impl BouncedHitMap {
    /// Add a hit point to this [`BouncedHitMap`].
    pub fn add_to_hitmap(&mut self, hit_point: HitPoint, uuid: &Uuid) {
        if let Some(rays_hit_map) = self.hit_map.get_mut(uuid) {
            rays_hit_map.add_hit_point(hit_point);
        } else {
            self.hit_map.insert(*uuid, RaysHitMap::new(&[hit_point]));
        }
    }
    /// Returns a reference to a [`RaysHitMap`] in this [`BouncedHitMap`]
    #[must_use]
    pub fn get_rays_hit_map(&self, uuid: &Uuid) -> Option<&RaysHitMap> {
        self.hit_map.get(uuid)
    }
}
/// A hit point as part of a [`RaysHitMap`].
///
/// It stores the position (intersection point) and the energy of a [`Ray`](crate::ray::Ray) that
/// has hit an [`OpticSurface`](crate::surface::optic_surface::OpticSurface)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitPoint {
    position: Point3<Length>,
    energy: Energy,
}
impl HitPoint {
    /// Create a new [`HitPoint`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the given energy is negative or not finite.
    ///   - the position coordinates (x/y/z) are not finite.
    pub fn new(position: Point3<Length>, energy: Energy) -> OpmResult<Self> {
        if !energy.is_finite() | energy.is_sign_negative() {
            return Err(OpossumError::Other(
                "energy must be positive and finite".into(),
            ));
        }
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err(OpossumError::Other("position must be finite".into()));
        }
        Ok(Self { position, energy })
    }
    /// Returns the position of this [`HitPoint`].
    #[must_use]
    pub fn position(&self) -> Point3<Length> {
        self.position
    }
    /// Returns the energy of this [`HitPoint`].
    #[must_use]
    pub fn energy(&self) -> Energy {
        self.energy
    }
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
///Storage struct for hitpoints on a surface from a single ray bundle
pub struct RaysHitMap {
    hit_map: Vec<HitPoint>,
}
impl RaysHitMap {
    /// Creates a new [`RaysHitMap`]
    fn new(hit_points: &[HitPoint]) -> Self {
        let mut hps = Vec::with_capacity(hit_points.len());
        for hit_point in hit_points {
            hps.push(hit_point.clone());
        }
        Self { hit_map: hps }
    }
    /// Add intersection point (with energy) to this [`HitMap`].
    pub fn add_hit_point(&mut self, hit_point: HitPoint) {
        self.hit_map.push(hit_point);
    }
    /// Merge this [`RaysHitMap`] with another [`RaysHitMap`].
    pub fn merge(&mut self, other_map: &Self) {
        for hit_point in &other_map.hit_map {
            self.add_hit_point(hit_point.to_owned());
        }
    }
    /// Calculates the fluence of this [`RaysHitMap`] using the "Voronoi" method
    /// # Attributes
    /// - `max_fluence`: the maximum allowed fluence on this surface
    /// # Errors
    /// This function errors if no reasonable axlimits  can be estimated due to only non-finite values in the positions
    #[allow(clippy::type_complexity)]
    pub fn calc_fluence_with_voronoi(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        let mut pos_in_cm = MatrixXx2::<f64>::zeros(self.hit_map.len());
        let mut energy = DVector::<f64>::zeros(self.hit_map.len());
        // let mut energy_in_ray_bundle = 0.;

        if self.hit_map.len() < 3 {
            return Err(OpossumError::Other(
                "Too few points (<3) on hitmap to calculate fluence!".into(),
            ));
        }
        for (row, p) in self.hit_map.iter().enumerate() {
            pos_in_cm[(row, 0)] = p.position.x.get::<length::centimeter>();
            pos_in_cm[(row, 1)] = p.position.y.get::<length::centimeter>();
            energy[row] = p.energy.get::<joule>();
            // energy_in_ray_bundle += energy[row];
        }

        let proj_ax1_lim = AxLims::finite_from_dvector(&pos_in_cm.column(0)).ok_or_else(|| {
            OpossumError::Other(
                "cannot construct voronoi cells with non-finite axes bounds!".into(),
            )
        })?;
        let proj_ax2_lim = AxLims::finite_from_dvector(&pos_in_cm.column(1)).ok_or_else(|| {
            OpossumError::Other(
                "cannot construct voronoi cells with non-finite axes bounds!".into(),
            )
        })?;
        let (voronoi, _beam_area) = create_voronoi_cells(&pos_in_cm).map_err(|e| {
            OpossumError::Other(format!(
                "Voronoi diagram for fluence estimation could not be created!: {e}"
            ))
        })?;
        //get the voronoi cells
        let v_cells = voronoi.cells();
        let mut fluence_scatter = DVector::from_element(voronoi.sites.len(), f64::NAN);
        let mut max_fluence_val = 0.;
        for (i, v_cell) in v_cells.iter().enumerate() {
            let v_neighbours = v_cell
                .points()
                .iter()
                .map(|p| Point2::new(p.x, p.y))
                .collect::<Vec<Point2<f64>>>();
            if v_neighbours.len() >= 3 {
                let poly_area = calc_closed_poly_area(&v_neighbours)?;
                fluence_scatter[i] = energy[i] / poly_area;
                if max_fluence_val < fluence_scatter[i] || i == 0 {
                    max_fluence_val = fluence_scatter[i];
                }
            } else {
                warn!(
                    "polygon could not be created. number of neighbors {}",
                    v_neighbours.len()
                );
            }
        }

        //axes definition
        let co_ax1 = linspace(proj_ax1_lim.min, proj_ax1_lim.max, nr_of_points.0)?;
        let co_ax2 = linspace(proj_ax2_lim.min, proj_ax2_lim.max, nr_of_points.1)?;

        let voronied_data =
            VoronoiedData::combine_data_with_voronoi_diagram(voronoi, fluence_scatter)?;
        //currently only interpolation. voronoid data for plotting must still be implemented
        let (interp_fluence, _) =
            interpolate_3d_triangulated_scatter_data(&voronied_data, &co_ax1, &co_ax2)?;
        let fluence_matrix = DMatrix::from_iterator(
            co_ax1.len(),
            co_ax2.len(),
            interp_fluence.iter().map(|val| J_per_cm2!(*val)),
        );
        let fluence_data = FluenceData::new(
            fluence_matrix,
            centimeter!(proj_ax1_lim.min)..centimeter!(proj_ax1_lim.max),
            centimeter!(proj_ax2_lim.min)..centimeter!(proj_ax2_lim.max),
        );
        Ok(fluence_data)
    }
    fn calc_2d_bounding_box(&self, margin: Length) -> OpmResult<(Length, Length, Length, Length)> {
        if !margin.is_finite() {
            return Err(OpossumError::Other("margin must be finite".into()));
        }
        self.hit_map.first().map_or_else(
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
                for point in &self.hit_map {
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
    /// Returns the calc fluence with kde of this [`RaysHitMap`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn calc_fluence_with_kde(&self, nr_of_points: (usize, usize)) -> OpmResult<FluenceData> {
        let mut kde = Kde::default();
        let hitmap_2d = self
            .hit_map
            .iter()
            .map(|p| (p.position.xy(), p.energy))
            .collect();
        kde.set_hit_map(hitmap_2d);
        let est_bandwidth = kde.bandwidth_estimate();
        kde.set_band_width(est_bandwidth);
        let (left, right, top, bottom) = self.calc_2d_bounding_box(3. * est_bandwidth)?;
        let fluence_matrix = kde.kde_2d(&(left..right, bottom..top), nr_of_points);
        let fluence_data = FluenceData::new(fluence_matrix, left..right, bottom..top);
        Ok(fluence_data)
    }
    /// Calculate a fluence map ([`FluenceData`]) of this [`RaysHitMap`].
    ///
    /// Create a fluence map with the given number of points and the concrete estimator algorithm.
    ///
    /// # Errors
    ///
    /// This function will return an error if  the underlying concrete estimator function returns an error.
    pub fn calc_fluence_map(
        &self,
        nr_of_points: (usize, usize),
        estimator: &FluenceEstimator,
    ) -> OpmResult<FluenceData> {
        match estimator {
            FluenceEstimator::Voronoi => self.calc_fluence_with_voronoi(nr_of_points),
            FluenceEstimator::KDE => self.calc_fluence_with_kde(nr_of_points),
            FluenceEstimator::Binning => todo!(),
        }
    }
}

/// Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
/// [`OpticSurface`](crate::surface::optic_surface::OpticSurface).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct HitMap {
    /// Stores the hitpoints of the rays on this surface, separated by their bounce level and the individual ray bundle
    hit_map: Vec<BouncedHitMap>,
    /// Stores the fluence, position in the history of the ray bundles that create a critical fluence on this surface and the bounce level. key value is the uuid of the ray bundle
    critical_fluence: HashMap<Uuid, (Fluence, usize, usize)>,
}
impl HitMap {
    /// Returns a reference to the hit map of this [`HitMap`].
    ///
    /// This function returns a vector of intersection points (with energies) of [`Rays`](crate::rays::Rays) that hit the surface.
    #[must_use]
    pub fn hit_map(&self) -> &[BouncedHitMap] {
        &self.hit_map
    }
    /// Add intersection point (with energy) to this [`HitMap`].
    pub fn add_to_hitmap(&mut self, hit_point: HitPoint, bounce: usize, uuid: &Uuid) {
        // make sure that vector is large enough to insert the data
        if self.hit_map.len() <= bounce {
            for _i in 0..bounce + 1 - self.hit_map.len() {
                self.hit_map.push(BouncedHitMap::default());
            }
        }
        self.hit_map[bounce].add_to_hitmap(hit_point, uuid);
    }
    /// Reset this [`HitMap`].
    ///
    /// This functions clears all point of the map.
    pub fn reset(&mut self) {
        self.hit_map.clear();
    }
    /// Returns `true` the [`HitMap`] is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hit_map.is_empty()
    }
    /// returns a reference to the `critical_fluence` field of this [`HitMap`] which contains:
    /// - the uuid of the ray bundle that causes this critical fluence on this surface as key
    /// - a tuple containing the calculated peak fluence, the index of the position history and the bounce level which can be used to reconstruct the ray-propagation plot later on
    #[must_use]
    pub fn critical_fluences(&self) -> &HashMap<Uuid, (Fluence, usize, usize)> {
        &self.critical_fluence
    }

    ///stores a critical fluence in a hitmap
    pub fn add_critical_fluence(
        &mut self,
        uuid: &Uuid,
        rays_hist_pos: usize,
        fluence: Fluence,
        bounce: usize,
    ) {
        self.critical_fluence
            .insert(*uuid, (fluence, rays_hist_pos, bounce));
    }

    ///returns a reference to a [`RaysHitMap`] in this [`HitMap`]
    #[must_use]
    pub fn get_rays_hit_map(&self, bounce: usize, uuid: &Uuid) -> Option<&RaysHitMap> {
        if bounce >= self.hit_map.len() {
            None
        } else {
            self.hit_map[bounce].get_rays_hit_map(uuid)
        }
    }
    /// Returns a merged [`RaysHitMap`] containing all bounces and uuid's of this [`HitMap`].
    #[must_use]
    pub fn get_merged_rays_hit_map(&self) -> RaysHitMap {
        let mut merged_rays_hit_map = RaysHitMap::default();
        for bounced_hit_map in &self.hit_map {
            for hit_map in &bounced_hit_map.hit_map {
                merged_rays_hit_map.merge(hit_map.1);
            }
        }
        merged_rays_hit_map
    }
}
impl From<HitMap> for Proptype {
    fn from(value: HitMap) -> Self {
        Self::HitMap(value)
    }
}
impl Plottable for HitMap {
    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        //ray plot series
        if self.hit_map.is_empty() {
            Ok(None)
        } else {
            let mut plt_series = Vec::<PlotSeries>::with_capacity(self.hit_map.len());
            let mut xy_positions = Vec::<Vec<Point2<Length>>>::with_capacity(self.hit_map.len());
            let mut x_max = f64::NEG_INFINITY;
            let mut y_max = f64::NEG_INFINITY;
            let mut x_min = f64::INFINITY;
            let mut y_min = f64::INFINITY;

            for (i, bounced_ray_bundles) in self.hit_map.iter().enumerate() {
                xy_positions.push(Vec::<Point2<Length>>::new());
                for rays_hitmap in bounced_ray_bundles.hit_map.values() {
                    for p in &rays_hitmap.hit_map {
                        xy_positions[i].push(Point2::new(p.position.x, p.position.y));

                        x_max = x_max.max(p.position.x.value);
                        y_max = y_max.max(p.position.y.value);
                        x_min = x_min.min(p.position.x.value);
                        y_min = y_min.min(p.position.y.value);
                    }
                }
            }
            let x_exponent = get_exponent_for_base_unit_in_e3_steps(x_max);
            let y_exponent = get_exponent_for_base_unit_in_e3_steps(y_max);
            let y_prefix = get_prefix_for_base_unit(y_max);
            let x_prefix = get_prefix_for_base_unit(x_max);

            plt_type.set_plot_param(&PlotArgs::XLabel(format!("x position ({y_prefix}m)")))?;
            plt_type.set_plot_param(&PlotArgs::YLabel(format!("y position ({x_prefix}m)")))?;

            for (i, xy_pos) in xy_positions.iter().enumerate() {
                if xy_pos.is_empty() {
                    continue;
                }
                let x_vals = xy_pos
                    .iter()
                    .map(|p| get_unit_value_as_length_with_format_by_exponent(p.x, x_exponent))
                    .collect::<Vec<f64>>();
                let y_vals = xy_pos
                    .iter()
                    .map(|p| get_unit_value_as_length_with_format_by_exponent(p.y, y_exponent))
                    .collect::<Vec<f64>>();

                let data = PlotData::Dim2 {
                    xy_data: MatrixXx2::from_columns(&[
                        DVector::from_vec(x_vals),
                        DVector::from_vec(y_vals),
                    ]),
                };

                let gradient = colorous::TURBO;
                let c = if self.hit_map.len() > 10 {
                    gradient.eval_rational(i, self.hit_map.len())
                } else {
                    colorous::CATEGORY10[i]
                };
                let label = format!("Bounce: {i}");
                plt_series.push(PlotSeries::new(
                    &data,
                    RGBAColor(c.r, c.g, c.b, 1.),
                    Some(label),
                ));
            }

            x_max *= f64::powi(10., -x_exponent);
            y_max *= f64::powi(10., -y_exponent);
            x_min *= f64::powi(10., -x_exponent);
            y_min *= f64::powi(10., -y_exponent);

            let x_diff = x_max - x_min;
            let y_diff = y_max - y_min;
            let x_limits = AxLims::create_useful_axlims(
                0.1f64.mul_add(-x_diff, x_min),
                0.1f64.mul_add(x_diff, x_max),
            );
            let y_limits = AxLims::create_useful_axlims(
                0.1f64.mul_add(-y_diff, y_min),
                0.1f64.mul_add(y_diff, y_max),
            );

            plt_type.set_plot_param(&PlotArgs::XLim(x_limits))?;
            plt_type.set_plot_param(&PlotArgs::YLim(y_limits))?;
            Ok(Some(plt_series))
        }
    }
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::AxisEqual(true))?
            .set(&PlotArgs::PlotAutoSize(true))?
            .set(&PlotArgs::PlotSize((800, 800)))?;
        Ok(())
    }
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::Scatter2D(plt_params.clone())
    }
}

#[cfg(test)]
mod test_hitpoint {
    use super::HitPoint;
    use crate::{joule, meter};
    use core::f64;
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
    fn getters() {
        let hm = HitPoint::new(meter!(1.0, 2.0, 3.0), joule!(4.0)).unwrap();
        assert_eq!(hm.position().x, meter!(1.0));
        assert_eq!(hm.position().y, meter!(2.0));
        assert_eq!(hm.position().z, meter!(3.0));
        assert_eq!(hm.energy(), joule!(4.0));
    }
}
#[cfg(test)]
mod test_rays_hit_map {
    use super::RaysHitMap;
    use crate::{joule, meter, surface::hit_map::HitPoint};
    use core::f64;
    #[test]
    fn new() {
        let rhm = RaysHitMap::new(&vec![]);
        assert_eq!(rhm.hit_map.len(), 0);

        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        let rhm = RaysHitMap::new(&vec![hp]);
        assert_eq!(rhm.hit_map.len(), 1);
    }
    #[test]
    fn add_to_hitmap() {
        let mut rhm = RaysHitMap::default();
        assert_eq!(rhm.hit_map.len(), 0);
        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        rhm.add_hit_point(hp);
        assert_eq!(rhm.hit_map.len(), 1);
    }
    #[test]
    fn merge() {
        let hp = HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap();
        let mut rhm = RaysHitMap::new(&vec![hp]);
        let hp2 = HitPoint::new(meter!(1.0, 1.0, 1.0), joule!(1.0)).unwrap();
        let rhm2 = RaysHitMap::new(&vec![hp2]);
        rhm.merge(&rhm2);
        assert_eq!(rhm.hit_map.len(), 2);
    }
    #[test]
    fn calc_2d_bounding_box() {
        let mut rhm = RaysHitMap::default();
        assert!(rhm.calc_2d_bounding_box(meter!(0.0)).is_err());
        rhm.add_hit_point(HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0)).unwrap(),
            (meter!(0.0), meter!(0.0), meter!(0.0), meter!(0.0))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.0)).unwrap(),
            (meter!(-1.0), meter!(0.0), meter!(1.0), meter!(0.0))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 1.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-0.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, -1.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(1.5), meter!(-1.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(-1.0, 2.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(0.5), meter!(2.5), meter!(-1.5))
        );
        rhm.add_hit_point(HitPoint::new(meter!(1.0, 2.0, 0.0), joule!(1.0)).unwrap());
        assert_eq!(
            rhm.calc_2d_bounding_box(meter!(0.5)).unwrap(),
            (meter!(-1.5), meter!(1.5), meter!(2.5), meter!(-1.5))
        );
        assert!(rhm.calc_2d_bounding_box(meter!(f64::NAN)).is_err());
        assert!(rhm.calc_2d_bounding_box(meter!(f64::INFINITY)).is_err());
        assert!(rhm.calc_2d_bounding_box(meter!(f64::NEG_INFINITY)).is_err());
    }
}

#[cfg(test)]
mod test_bounced_hit_map {
    use crate::{
        joule, meter,
        surface::hit_map::{BouncedHitMap, HitPoint},
    };
    use uuid::Uuid;

    #[test]
    fn add_to_hitmap() {
        let mut bhm = BouncedHitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            &uuid1,
        );
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map.len(), 1);
        assert!(bhm.hit_map.get(&uuid2).is_none());
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            &uuid1,
        );
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map.len(), 2);
        assert!(bhm.hit_map.get(&uuid2).is_none());
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            &uuid2,
        );
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map.len(), 2);
        assert_eq!(bhm.hit_map.get(&uuid2).unwrap().hit_map.len(), 1);
    }
    #[test]
    fn get_rays_hit_map() {
        let mut bhm = BouncedHitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            &uuid1,
        );
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            &uuid1,
        );
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            &uuid2,
        );
        assert_eq!(bhm.get_rays_hit_map(&uuid1).unwrap().hit_map.len(), 2);
        assert_eq!(bhm.get_rays_hit_map(&uuid2).unwrap().hit_map.len(), 1);
        assert!(bhm.get_rays_hit_map(&Uuid::nil()).is_none());
    }
}
#[cfg(test)]
mod test_hit_map {
    use uuid::Uuid;

    use crate::{
        joule, meter,
        properties::Proptype,
        surface::hit_map::{HitMap, HitPoint},
    };

    #[test]
    fn hit_map() {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &uuid1,
        );
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &uuid2,
        );
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            0,
            &uuid1,
        );
        assert_eq!(hm.hit_map().len(), 2);
    }
    #[test]
    fn add_to_hitmap() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &Uuid::new_v4(),
        );
        assert_eq!(hm.hit_map.len(), 2);
        assert_eq!(hm.hit_map[0].hit_map.len(), 0);
        assert_eq!(hm.hit_map[1].hit_map.len(), 1);
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            0,
            &Uuid::new_v4(),
        );
        assert_eq!(hm.hit_map.len(), 2);
        assert_eq!(hm.hit_map[0].hit_map.len(), 1);
        assert_eq!(hm.hit_map[1].hit_map.len(), 1);
    }
    #[test]
    fn reset() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &Uuid::new_v4(),
        );
        hm.reset();
        assert!(hm.is_empty());
    }
    #[test]
    fn is_empty() {
        let mut hm = HitMap::default();
        assert!(hm.is_empty());
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &Uuid::new_v4(),
        );
        assert!(!hm.is_empty());
    }
    #[test]
    fn get_rays_hit_map() {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &uuid1,
        );
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &uuid2,
        );
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            0,
            &uuid1,
        );
        assert!(hm.get_rays_hit_map(2, &uuid1).is_none());
        assert!(hm.get_rays_hit_map(0, &uuid2).is_none());
    }
    #[test]
    fn get_merged_rays_hit_map() {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &uuid1,
        );
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            1,
            &uuid2,
        );
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            0,
            &uuid1,
        );
        assert_eq!(hm.get_merged_rays_hit_map().hit_map.len(), 3);
    }
    #[test]
    fn proptype_from() {
        let prop_type: Proptype = HitMap::default().into();
        assert!(matches!(prop_type, Proptype::HitMap(_)));
    }
}
