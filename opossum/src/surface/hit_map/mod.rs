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
    error::OpmResult,
    nodes::fluence_detector::Fluence,
    plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::Proptype,
    utils::unit_format::{
        get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit,
        get_unit_value_as_length_with_format_by_exponent,
    },
};
use nalgebra::{DVector, MatrixXx2, Point2};
use plotters::style::RGBAColor;
use rays_hit_map::{HitPoint, RaysHitMap};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uom::si::f64::Length;
use uuid::Uuid;

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
                    for p in rays_hitmap.hit_map() {
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
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map().len(), 1);
        assert!(bhm.hit_map.get(&uuid2).is_none());
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            &uuid1,
        );
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map().len(), 2);
        assert!(bhm.hit_map.get(&uuid2).is_none());
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0)).unwrap(),
            &uuid2,
        );
        assert_eq!(bhm.hit_map.get(&uuid1).unwrap().hit_map().len(), 2);
        assert_eq!(bhm.hit_map.get(&uuid2).unwrap().hit_map().len(), 1);
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
        assert_eq!(bhm.get_rays_hit_map(&uuid1).unwrap().hit_map().len(), 2);
        assert_eq!(bhm.get_rays_hit_map(&uuid2).unwrap().hit_map().len(), 1);
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
        assert_eq!(hm.get_merged_rays_hit_map().hit_map().len(), 3);
    }
    #[test]
    fn proptype_from() {
        let prop_type: Proptype = HitMap::default().into();
        assert!(matches!(prop_type, Proptype::HitMap(_)));
    }
}
