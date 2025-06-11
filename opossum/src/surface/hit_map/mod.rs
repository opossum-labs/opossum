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

pub mod fluence_estimator;
pub mod rays_hit_map;

use crate::{
    J_per_cm2,
    error::{OpmResult, OpossumError},
    meter,
    nodes::fluence_detector::{Fluence, fluence_data::FluenceData},
    plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::Proptype,
    utils::unit_format::{
        get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
        get_unit_value_as_length_with_format_by_exponent,
    },
};
use fluence_estimator::FluenceEstimator;
use log::warn;
use nalgebra::{DMatrix, DVector, MatrixXx2, Point2};
use plotters::style::RGBAColor;
use rays_hit_map::{HitPoint, HitPoints, RaysHitMap};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Range};
use uom::si::f64::Length;
use uuid::Uuid;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Storage struct for `RaysHitMap` on a surface from a single bounce
pub struct BouncedHitMap {
    hit_map: HashMap<Uuid, RaysHitMap>,
}
impl BouncedHitMap {
    /// Add a hit point to this [`BouncedHitMap`].
    ///
    /// # Errors
    /// This function errors if the hit-point that should be added does not match the already stored hit point type
    pub fn add_to_hitmap(&mut self, hit_point: HitPoint, uuid: Uuid) -> OpmResult<()> {
        if let Some(rays_hit_map) = self.hit_map.get_mut(&uuid) {
            rays_hit_map.add_hit_point(hit_point)?;
        } else {
            match hit_point {
                HitPoint::Energy(energy_hit_point) => {
                    let mut rhm = RaysHitMap::new(HitPoints::Energy(vec![]));
                    rhm.add_hit_point(HitPoint::Energy(energy_hit_point))?;
                    self.hit_map.insert(uuid, rhm);
                }
                HitPoint::Fluence(fluence_hit_point) => {
                    let mut rhm = RaysHitMap::new(HitPoints::Fluence(vec![]));
                    rhm.add_hit_point(HitPoint::Fluence(fluence_hit_point))?;
                    self.hit_map.insert(uuid, rhm);
                }
            }
        }
        Ok(())
    }
    /// Returns a reference to a [`RaysHitMap`] in this [`BouncedHitMap`]
    #[must_use]
    pub fn get_rays_hit_map(&self, uuid: Uuid) -> Option<&RaysHitMap> {
        self.hit_map.get(&uuid)
    }
}

/// Data structure for storing intersection points (and energies) of [`Rays`](crate::rays::Rays) hitting an
/// [`OpticSurface`](crate::surface::optic_surface::OpticSurface).
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    #[allow(clippy::missing_const_for_fn)]
    pub fn hit_map(&self) -> &[BouncedHitMap] {
        &self.hit_map
    }
    /// Add intersection point to this [`HitMap`].
    ///
    /// # Errors
    /// This function errors if adding the hit point to the [`BouncedHitMap`] fails
    pub fn add_to_hitmap(
        &mut self,
        hit_point: HitPoint,
        bounce: usize,
        uuid: Uuid,
    ) -> OpmResult<()> {
        // make sure that vector is large enough to insert the data
        if self.hit_map.len() <= bounce {
            for _i in 0..bounce + 1 - self.hit_map.len() {
                self.hit_map.push(BouncedHitMap::default());
            }
        }
        self.hit_map[bounce].add_to_hitmap(hit_point, uuid)?;
        Ok(())
    }

    /// Reset this [`HitMap`].
    ///
    /// This functions clears all point of the map.
    pub fn reset(&mut self) {
        self.hit_map.clear();
    }
    /// Returns `true` the [`HitMap`] is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
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
        uuid: Uuid,
        rays_hist_pos: usize,
        fluence: Fluence,
        bounce: usize,
    ) {
        self.critical_fluence
            .insert(uuid, (fluence, rays_hist_pos, bounce));
    }

    ///returns a reference to a [`RaysHitMap`] in this [`HitMap`]
    #[must_use]
    pub fn get_rays_hit_map(&self, bounce: usize, uuid: Uuid) -> Option<&RaysHitMap> {
        if bounce >= self.hit_map.len() {
            None
        } else {
            self.hit_map[bounce].get_rays_hit_map(uuid)
        }
    }
    /// Returns a merged [`RaysHitMap`] containing all bounces and uuid's of this [`HitMap`].
    ///
    /// # Errors
    /// This function errors if merging the [`RaysHitMap`] fails
    pub fn get_merged_rays_hit_map(&self) -> OpmResult<RaysHitMap> {
        let mut merged_rays_hit_map = RaysHitMap::default();
        let mut count = 0;
        for bounced_hit_map in &self.hit_map {
            for hit_map in &bounced_hit_map.hit_map {
                if count == 0 {
                    merged_rays_hit_map = hit_map.1.clone();
                } else {
                    merged_rays_hit_map.merge(hit_map.1)?;
                }
                count += 1;
            }
        }
        Ok(merged_rays_hit_map)
    }

    /// Returns the 'bounding box' of this hitmap, meaning the minimum and maximum position values in x and y
    #[must_use]
    pub fn get_bounding_box(&self) -> (Range<Length>, Range<Length>) {
        let mut x_min = meter!(0.);
        let mut x_max = meter!(0.);
        let mut y_min = meter!(0.);
        let mut y_max = meter!(0.);
        let mut count = 0;
        for bounced_hit_map in &self.hit_map {
            for hit_map in bounced_hit_map.hit_map.values() {
                if count == 0 {
                    (x_min, x_max) = *hit_map.x_lims();
                    (y_min, y_max) = *hit_map.y_lims();
                } else {
                    let x_lims = hit_map.x_lims();
                    let y_lims = hit_map.y_lims();
                    x_min = x_min.min(x_lims.0);
                    x_max = x_max.max(x_lims.1);
                    y_min = y_min.min(y_lims.0);
                    y_max = y_max.max(y_lims.1);
                }
                count += 1;
            }
        }
        (x_min..x_max, y_min..y_max)
    }

    /// Returns the first hitpoint in this [`HitMap`] or None if there is none
    #[must_use]
    pub fn get_first_hitpoints(&self) -> Option<&HitPoints> {
        for bhm in &self.hit_map {
            if bhm.hit_map.is_empty() {
                continue;
            }
            if let Some(rhm) = bhm.hit_map.values().next() {
                return Some(rhm.hit_map());
            }
        }
        None
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`HitMap`] using the "Voronoi" method
    ///
    /// This method tries to combine the fluence of data of all stored [`RaysHitMap`]s and return a single [`FluenceData`]
    ///
    /// # Note
    /// The resulting fluence map may be inaccurate for the combination of large beams with small beams, since, currently, the method relies solely on interpolation.
    /// This is problematic if one of the beam sizes is in the order of one pixel of the matrix or even below.
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    ///
    /// # Errors
    /// This function errors if
    /// - The hit point type is neither energy nor fluence
    pub fn calc_combined_fluence_with_voronoi(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        let hit_point_opt = &self.get_first_hitpoints();
        if let Some(HitPoints::Energy(_)) = hit_point_opt {
            let (ax_1_range, ax_2_range) = self.get_bounding_box();
            let mut fluence_matrix =
                DMatrix::from_element(nr_of_points.0, nr_of_points.1, J_per_cm2!(0.));

            for bounced_hit_map in &self.hit_map {
                for rays_hit_map in bounced_hit_map.hit_map.values() {
                    let fl_data = rays_hit_map.calc_fluence_with_voronoi(
                        nr_of_points,
                        Some(&ax_1_range),
                        Some(&ax_2_range),
                    )?;
                    fluence_matrix += fl_data.interp_distribution();
                }
            }

            Ok(FluenceData::new(
                fluence_matrix,
                ax_1_range,
                ax_2_range,
                FluenceEstimator::Voronoi,
            ))
        } else if let Some(HitPoints::Fluence(_)) = hit_point_opt {
            warn!(
                "Unexpected type of HitPoints for voronoi estimator! Changing to helper-ray estimator!"
            );
            self.calc_combined_fluence_with_helper_rays(nr_of_points)
        } else {
            Err(OpossumError::Analysis("Wrong hit point type to calculate fluence with voronoi estimator! Must be an EnergyHitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`HitMap`] using the "Helper Rays" method
    ///
    /// This method tries to combine the fluence of data of all stored [`RaysHitMap`]s and return a single [`FluenceData`]
    ///
    /// # Note
    /// The resulting fluence map may be inaccurate for the combination of large beams with small beams, since, currently, the method relies solely on interpolation.
    /// This is problematic if one of the beam sizes is in the order of one pixel of the matrix or even below.
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    ///
    /// # Errors
    /// This function errors if
    /// - The hit point type is neither energy nor fluence
    pub fn calc_combined_fluence_with_helper_rays(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        let hit_point_opt = &self.get_first_hitpoints();
        if let Some(HitPoints::Fluence(_)) = hit_point_opt {
            let (ax_1_range, ax_2_range) = self.get_bounding_box();
            let mut fluence_matrix =
                DMatrix::from_element(nr_of_points.0, nr_of_points.1, J_per_cm2!(0.));

            for bounced_hit_map in &self.hit_map {
                for rays_hit_map in bounced_hit_map.hit_map.values() {
                    let fl_data = rays_hit_map.calc_fluence_with_helper_rays(
                        nr_of_points,
                        Some(&ax_1_range),
                        Some(&ax_2_range),
                    )?;
                    fluence_matrix += fl_data.interp_distribution();
                }
            }

            Ok(FluenceData::new(
                fluence_matrix,
                ax_1_range,
                ax_2_range,
                FluenceEstimator::HelperRays,
            ))
        } else if let Some(HitPoints::Energy(_)) = hit_point_opt {
            warn!(
                "Unexpected type of HitPoints for helper-ray estimator! Changing to voronoi estimator!"
            );
            self.calc_combined_fluence_with_voronoi(nr_of_points)
        } else {
            Err(OpossumError::Analysis("Wrong hit point type to calculate fluence with helper-ray estimator! Must be a FluenceHitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`HitMap`] using the "Kernel Density Estimator (KDE)" method
    ///
    /// This method tries to combine the fluence of data of all stored [`RaysHitMap`]s and return a single [`FluenceData`]
    ///
    /// # Note
    /// The resulting fluence map may be inaccurate for the combination of large beams with small beams, since the same kernel size is used for both beams
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    ///
    /// # Errors
    /// This function errors if
    /// - the [`RaysHitMap`]s canot be merged.
    /// - The hit point type is neither energy nor fluence
    pub fn calc_combined_fluence_with_kde(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        let hit_point_opt = &self.get_first_hitpoints();
        if let Some(HitPoints::Energy(_)) = hit_point_opt {
            self.get_merged_rays_hit_map()?
                .calc_fluence_with_kde(nr_of_points, None, None)
        } else if let Some(HitPoints::Fluence(_)) = hit_point_opt {
            warn!(
                "Unexpected type of HitPoints for kernel density estimator! Changing to helper-ray estimator!"
            );
            self.calc_combined_fluence_with_helper_rays(nr_of_points)
        } else {
            Err(OpossumError::Analysis("Wrong hit point type to calculate fluence with kernel density estimator! Must be an EnergyHitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`HitMap`] using the "Binning" method
    ///
    /// This method tries to combine the fluence of data of all stored [`RaysHitMap`]s and return a single [`FluenceData`]
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    ///
    /// # Errors
    /// This function errors if
    /// - the [`RaysHitMap`]s canot be merged.
    /// - The hit point type is neither energy nor fluence
    pub fn calc_combined_fluence_with_binning(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        let hit_point_opt = &self.get_first_hitpoints();
        if let Some(HitPoints::Energy(_)) = hit_point_opt {
            self.get_merged_rays_hit_map()?
                .calc_fluence_with_binning(nr_of_points, None, None)
        } else if let Some(HitPoints::Fluence(_)) = hit_point_opt {
            warn!(
                "Unexpected type of HitPoints for binning estimator! Changing to helper-ray estimator!"
            );
            self.calc_combined_fluence_with_helper_rays(nr_of_points)
        } else {
            Err(OpossumError::Analysis("Wrong hit point type to calculate fluence with binning estimator! Must be an EnergyHitpoint!".into()))
        }
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`HitMap`].
    ///
    /// Create a fluence map with the given number of points and the concrete estimator algorithm.
    ///
    /// # Errors
    ///
    /// This function will return an error if the underlying concrete estimator function returns an error.
    pub fn calc_fluence_map(
        &self,
        nr_of_points: (usize, usize),
        estimator: &FluenceEstimator,
    ) -> OpmResult<FluenceData> {
        match estimator {
            FluenceEstimator::Voronoi => self.calc_combined_fluence_with_voronoi(nr_of_points),
            FluenceEstimator::KDE => self.calc_combined_fluence_with_kde(nr_of_points),
            FluenceEstimator::Binning => self.calc_combined_fluence_with_binning(nr_of_points),
            FluenceEstimator::HelperRays => {
                self.calc_combined_fluence_with_helper_rays(nr_of_points)
            }
        }
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
                    for p in rays_hitmap.hit_map().positions() {
                        xy_positions[i].push(Point2::new(p.x, p.y));
                        x_max = x_max.max(p.x.value);
                        y_max = y_max.max(p.y.value);
                        x_min = x_min.min(p.x.value);
                        y_min = y_min.min(p.y.value);
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
mod test_bounced_hit_map {
    use crate::{
        joule, meter,
        surface::hit_map::{BouncedHitMap, HitPoint, rays_hit_map::EnergyHitPoint},
    };
    use uuid::Uuid;

    #[test]
    fn add_to_hitmap() {
        let mut bhm = BouncedHitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        bhm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            uuid1,
        )
        .unwrap();
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map().len(), 1);
        assert!(bhm.hit_map.get(&uuid2).is_none());
        bhm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            uuid1,
        )
        .unwrap();
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map().len(), 2);
        assert!(bhm.hit_map.get(&uuid2).is_none());
        bhm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            uuid2,
        )
        .unwrap();
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map().len(), 2);
        assert_eq!(bhm.hit_map.get(&uuid2).unwrap().hit_map().len(), 1);
    }
    #[test]
    fn get_rays_hit_map() {
        let mut bhm = BouncedHitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        bhm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            uuid1,
        )
        .unwrap();
        bhm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            uuid1,
        )
        .unwrap();
        bhm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            uuid2,
        )
        .unwrap();
        assert_eq!(bhm.get_rays_hit_map(uuid1).unwrap().hit_map().len(), 2);
        assert_eq!(bhm.get_rays_hit_map(uuid2).unwrap().hit_map().len(), 1);
        assert!(bhm.get_rays_hit_map(Uuid::nil()).is_none());
    }
}
#[cfg(test)]
mod test_hit_map {
    use approx::assert_relative_eq;
    use uuid::Uuid;

    use crate::{
        J_per_cm2, joule, meter,
        plottable::{PlotParameters, Plottable},
        properties::Proptype,
        surface::hit_map::{
            HitMap, HitPoint,
            fluence_estimator::FluenceEstimator,
            rays_hit_map::{EnergyHitPoint, FluenceHitPoint},
        },
        utils::test_helper::test_helper::check_logs,
    };

    #[test]
    fn hit_map() {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            1,
            uuid1,
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            1,
            uuid2,
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            0,
            uuid1,
        )
        .unwrap();
        assert_eq!(hm.hit_map().len(), 2);
    }

    #[test]
    fn add_wrong_to_hitmap_energy_same_bundle() {
        let uuid = Uuid::new_v4();
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            0,
            uuid,
        )
        .unwrap();
        assert!(
            hm.add_to_hitmap(
                HitPoint::Fluence(
                    FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap()
                ),
                0,
                uuid,
            )
            .is_err()
        );
    }

    #[test]
    fn add_wrong_to_hitmap_fluence_same_bundle() {
        let uuid = Uuid::new_v4();
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            0,
            uuid,
        )
        .unwrap();

        assert!(
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
                0,
                uuid,
            )
            .is_err()
        );
    }

    #[test]
    fn add_wrong_to_hitmap_different_bundle() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            0,
            Uuid::new_v4(),
        )
        .unwrap();

        assert!(
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
                0,
                Uuid::new_v4(),
            )
            .is_ok()
        );
    }

    #[test]
    fn add_wrong_to_hitmap_different_bounce() {
        let uuid = Uuid::new_v4();
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            0,
            uuid,
        )
        .unwrap();

        assert!(
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .is_ok()
        );
    }

    #[test]
    fn add_to_hitmap_energy() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        assert_eq!(hm.hit_map.len(), 2);
        assert_eq!(hm.hit_map[0].hit_map.len(), 0);
        assert_eq!(hm.hit_map[1].hit_map.len(), 1);
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            0,
            Uuid::new_v4(),
        )
        .unwrap();
        assert_eq!(hm.hit_map.len(), 2);
        assert_eq!(hm.hit_map[0].hit_map.len(), 1);
        assert_eq!(hm.hit_map[1].hit_map.len(), 1);
    }
    #[test]
    fn add_to_hitmap_fluence() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        assert_eq!(hm.hit_map.len(), 2);
        assert_eq!(hm.hit_map[0].hit_map.len(), 0);
        assert_eq!(hm.hit_map[1].hit_map.len(), 1);
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            0,
            Uuid::new_v4(),
        )
        .unwrap();
        assert_eq!(hm.hit_map.len(), 2);
        assert_eq!(hm.hit_map[0].hit_map.len(), 1);
        assert_eq!(hm.hit_map[1].hit_map.len(), 1);
    }
    #[test]
    fn reset_energy() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        hm.reset();
        assert!(hm.is_empty());
    }
    #[test]
    fn reset_fluence() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        hm.reset();
        assert!(hm.is_empty());
    }
    #[test]
    fn is_empty() {
        let mut hm = HitMap::default();
        assert!(hm.is_empty());
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        assert!(!hm.is_empty());
    }
    #[test]
    fn get_rays_hit_map() {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            uuid1,
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            uuid2,
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            0,
            uuid1,
        )
        .unwrap();
        assert!(hm.get_rays_hit_map(2, uuid1).is_none());
        assert!(hm.get_rays_hit_map(0, uuid2).is_none());
    }
    #[test]
    fn get_merged_rays_hit_map() {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            uuid1,
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            uuid2,
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            0,
            uuid1,
        )
        .unwrap();
        assert_eq!(hm.get_merged_rays_hit_map().unwrap().hit_map().len(), 3);
    }

    #[test]
    fn get_merged_rays_hit_map_mixed() {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            uuid1,
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap()),
            1,
            uuid2,
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(0.0, 0.0, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            0,
            uuid1,
        )
        .unwrap();
        assert!(hm.get_merged_rays_hit_map().is_err());
    }
    #[test]
    fn proptype_from() {
        let prop_type: Proptype = HitMap::default().into();
        assert!(matches!(prop_type, Proptype::HitMap(_)));
    }

    #[test]
    fn get_bounding_box() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.1, 1.0, 0.0), joule!(1.0)).unwrap()),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(-20.0, 20.0, 0.0), joule!(1.0)).unwrap()),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(3.210, 0.0, 0.0), joule!(1.0)).unwrap()),
            0,
            Uuid::new_v4(),
        )
        .unwrap();

        let (x_range, y_range) = hm.get_bounding_box();

        assert_eq!(x_range.start.value, -20.);
        assert_eq!(x_range.end.value, 3.21);
        assert_eq!(y_range.start.value, 0.0);
        assert_eq!(y_range.end.value, 20.);
    }
    #[test]
    fn get_first_hitpoints() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Energy(EnergyHitPoint::new(meter!(0.1, 1.0, 0.0), joule!(1.0)).unwrap()),
            1,
            Uuid::new_v4(),
        )
        .unwrap();

        let hp = hm.get_first_hitpoints();
        assert!(hp.is_some());
    }
    #[test]
    fn get_first_hitpoints_empty() {
        let hm = HitMap::default();
        let hp = hm.get_first_hitpoints();
        assert!(hp.is_none());
    }

    #[test]
    fn calc_combined_fluence_with_voronoi() {
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_voronoi((51, 51)).unwrap();
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 2.);

        let uuid = Uuid::new_v4();
        let pos2 = vec![
            meter!(f64::sqrt(0.5), 0., 0.),
            meter!(0., f64::sqrt(0.5), 0.),
            meter!(-f64::sqrt(0.5), 0., 0.),
            meter!(0., -f64::sqrt(0.5), 0.),
            meter!(0., 0., 0.),
        ];
        for pos in &pos2 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_voronoi((51, 51)).unwrap();
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 4.);
    }

    #[test]
    fn calc_combined_fluence_with_voronoi_too_few_points() {
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![meter!(-0.5, -0.5, 0.0), meter!(0., 0., 0.0)];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        assert!(hm.calc_combined_fluence_with_voronoi((50, 50)).is_err());
    }

    #[test]
    fn calc_combined_fluence_with_voronoi_wrong_hit_point_type() {
        testing_logger::setup();
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Fluence(FluenceHitPoint::new(pos.clone(), J_per_cm2!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_voronoi((51, 51)).unwrap();
        check_logs(
            log::Level::Warn,
            vec![
                "Unexpected type of HitPoints for voronoi estimator! Changing to helper-ray estimator!",
            ],
        );
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 10000.);
    }

    #[test]
    fn calc_combined_fluence_with_helper_rays() {
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Fluence(FluenceHitPoint::new(pos.clone(), J_per_cm2!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_helper_rays((51, 51)).unwrap();
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 10000.);

        let uuid = Uuid::new_v4();
        let pos2 = vec![
            meter!(f64::sqrt(0.5), 0., 0.),
            meter!(0., f64::sqrt(0.5), 0.),
            meter!(-f64::sqrt(0.5), 0., 0.),
            meter!(0., -f64::sqrt(0.5), 0.),
            meter!(0., 0., 0.),
        ];
        for pos in &pos2 {
            hm.add_to_hitmap(
                HitPoint::Fluence(FluenceHitPoint::new(pos.clone(), J_per_cm2!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_helper_rays((51, 51)).unwrap();
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 20000.);
    }

    #[test]
    fn calc_combined_fluence_with_helper_rays_too_few_points() {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(-0.5, -0.5, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        hm.add_to_hitmap(
            HitPoint::Fluence(
                FluenceHitPoint::new(meter!(-0.5, 0.5, 0.0), J_per_cm2!(1.0)).unwrap(),
            ),
            1,
            Uuid::new_v4(),
        )
        .unwrap();
        assert!(hm.calc_combined_fluence_with_helper_rays((50, 50)).is_err());
    }

    #[test]
    fn calc_combined_fluence_with_helper_rays_wrong_hit_point_type() {
        testing_logger::setup();
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_helper_rays((51, 51)).unwrap();
        check_logs(
            log::Level::Warn,
            vec![
                "Unexpected type of HitPoints for helper-ray estimator! Changing to voronoi estimator!",
            ],
        );
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 2.);
    }

    #[test]
    fn calc_combined_fluence_with_kde() {
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_kde((51, 51)).unwrap();
        assert_relative_eq!(
            fl_data.interp_distribution()[(25, 25)].value,
            5.474418964842738
        );

        let uuid = Uuid::new_v4();
        let pos2 = vec![
            meter!(f64::sqrt(0.5), 0., 0.),
            meter!(0., f64::sqrt(0.5), 0.),
            meter!(-f64::sqrt(0.5), 0., 0.),
            meter!(0., -f64::sqrt(0.5), 0.),
            meter!(0., 0., 0.),
        ];
        for pos in &pos2 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_kde((51, 51)).unwrap();
        assert_relative_eq!(
            fl_data.interp_distribution()[(25, 25)].value,
            8.969644069111087
        );
    }
    #[test]
    fn calc_combined_fluence_with_kde_wrong_hit_point_type() {
        testing_logger::setup();
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Fluence(FluenceHitPoint::new(pos.clone(), J_per_cm2!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_kde((51, 51)).unwrap();
        check_logs(
            log::Level::Warn,
            vec![
                "Unexpected type of HitPoints for kernel density estimator! Changing to helper-ray estimator!",
            ],
        );
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 10000.);
    }

    #[test]
    fn calc_combined_fluence_with_binning() {
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_binning((51, 51)).unwrap();
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 2601.0);

        let uuid = Uuid::new_v4();
        let pos2 = vec![
            meter!(f64::sqrt(0.5), 0., 0.),
            meter!(0., f64::sqrt(0.5), 0.),
            meter!(-f64::sqrt(0.5), 0., 0.),
            meter!(0., -f64::sqrt(0.5), 0.),
            meter!(0., 0., 0.),
        ];
        for pos in &pos2 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_binning((51, 51)).unwrap();
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 2601.0);
    }
    #[test]
    fn calc_combined_fluence_with_binning_wrong_hit_point_type() {
        testing_logger::setup();
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Fluence(FluenceHitPoint::new(pos.clone(), J_per_cm2!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let fl_data = hm.calc_combined_fluence_with_binning((51, 51)).unwrap();
        check_logs(
            log::Level::Warn,
            vec![
                "Unexpected type of HitPoints for binning estimator! Changing to helper-ray estimator!",
            ],
        );
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 10000.);
    }

    #[test]
    fn calc_fluence_map() {
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        assert!(
            hm.calc_fluence_map((51, 51), &FluenceEstimator::Voronoi)
                .is_ok()
        );
        assert!(
            hm.calc_fluence_map((51, 51), &FluenceEstimator::KDE)
                .is_ok()
        );
        assert!(
            hm.calc_fluence_map((51, 51), &FluenceEstimator::Binning)
                .is_ok()
        );
        assert!(
            hm.calc_fluence_map((51, 51), &FluenceEstimator::HelperRays)
                .is_ok()
        );
    }

    #[test]
    fn get_plot_series() {
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        let pos1 = vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ];
        for pos in &pos1 {
            hm.add_to_hitmap(
                HitPoint::Energy(EnergyHitPoint::new(pos.clone(), joule!(1.0)).unwrap()),
                1,
                uuid,
            )
            .unwrap();
        }
        let mut plt_params = PlotParameters::default();
        hm.add_plot_specific_params(&mut plt_params).unwrap();
        let plt_series = hm.get_plot_series(&mut hm.get_plot_type(&plt_params), false);
        assert!(plt_series.is_ok());
        let plt_series = plt_series.unwrap().unwrap();
        assert!(plt_series.len() == 1);
    }
}
