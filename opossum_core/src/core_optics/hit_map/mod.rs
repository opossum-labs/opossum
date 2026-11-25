//! Data structure for storing intersection points (and energies) of [`Rays`](crate::light::rays::Rays) hitting an
//! [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface).
//!
//! A [`HitMap`] not only stores the hit points but also the number of bounces a [`Ray`](crate::light::ray::Ray) has
//! undergone before hitting a surface and the [`Uuid`] of the ray bundle that caused the hit.
//!
//! The overall structure is a follows (in ascending hierarchy):
//!
//!  - The most basic structure is a [`HitPoint`] storing a [`Ray`s](crate::light::ray::Ray) intersection point with
//!    a surface and its energy.
//!  - A [`RaysHitMap`] simply stores a vector of [`HitPoint`]s. It also implements functions for calculating a fluence
//!    map (using either the Voronoi or the KDE method).
//!  - A [`BouncedHitMap`] stores a [`RaysHitMap`] together with an [`Uuid`] of the ray bundle ([`Rays`](crate::light::rays::Rays)).
//!  - A [`HitMap`] stores a vector of [`BouncedHitMap`]s. The vector index represents the number of ray bounces. So, the
//!    first entry contains all [`BouncedHitMap`]s caused by rays wih zero bounces, the second entry all [`BouncedHitMap`]s
//!    caused by rays wih one bounce, ...

pub mod fluence_estimator;
mod ray_tubes;
pub mod rays_hit_map;

use crate::{
    error::OpmResult,
    joule,
    light::RaySource,
    meter,
    nodes::fluence_detector::{Fluence, fluence_data::FluenceData},
    properties::Proptype,
};
use fluence_estimator::FluenceEstimator;
use itertools::Itertools;
use ray_tubes::draw_map;
use rays_hit_map::{HitPoint, RaysHitMap};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Range};
use uom::si::f64::{Energy, Length};
use uuid::Uuid;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Storage struct for `RaysHitMap` on a surface from a single bounce
pub struct BouncedHitMap {
    hit_map: HashMap<Uuid, RaysHitMap>,
}
impl BouncedHitMap {
    /// Add a hit point to this [`BouncedHitMap`].
    ///
    /// # Arguments
    ///
    /// * `hit_point` - where the ray hit and what it brought.
    /// * `uuid` - the bundle of the ray.
    /// * `source` - where the ray stands in the ray tubes of its source, if it does.
    ///
    /// # Errors
    /// This function errors if the hits stand in more than `u32::MAX` meshes (see
    /// [`RaysHitMap::add_hit_point`]).
    pub fn add_to_hitmap(
        &mut self,
        hit_point: HitPoint,
        uuid: Uuid,
        source: Option<&RaySource>,
    ) -> OpmResult<()> {
        self.hit_map
            .entry(uuid)
            .or_default()
            .add_hit_point(hit_point, source)
    }
    /// Returns a reference to the internal map of [`RaysHitMap`]s.
    #[must_use]
    pub const fn hit_map(&self) -> &HashMap<Uuid, RaysHitMap> {
        &self.hit_map
    }

    /// Returns a reference to a [`RaysHitMap`] in this [`BouncedHitMap`]
    #[must_use]
    pub fn get_rays_hit_map(&self, uuid: Uuid) -> Option<&RaysHitMap> {
        self.hit_map.get(&uuid)
    }
}

/// Data structure for storing intersection points (and energies) of [`Rays`](crate::light::rays::Rays) hitting an
/// [`OpticSurface`](crate::core_optics::optic_surface::OpticSurface).
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
    /// This function returns a vector of intersection points (with energies) of [`Rays`](crate::light::rays::Rays) that hit the surface.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn hit_map(&self) -> &[BouncedHitMap] {
        &self.hit_map
    }

    /// Add intersection point to this [`HitMap`].
    ///
    /// # Arguments
    ///
    /// * `hit_point` - where the ray hit and what it brought.
    /// * `bounce` - the number of bounces of the ray.
    /// * `uuid` - the bundle of the ray.
    /// * `source` - where the ray stands in the ray tubes of its source, if it does.
    ///
    /// # Errors
    /// This function errors if adding the hit point to the [`BouncedHitMap`] fails
    pub fn add_to_hitmap(
        &mut self,
        hit_point: HitPoint,
        bounce: usize,
        uuid: Uuid,
        source: Option<&RaySource>,
    ) -> OpmResult<()> {
        // make sure that vector is large enough to insert the data
        if self.hit_map.len() <= bounce {
            for _i in 0..bounce + 1 - self.hit_map.len() {
                self.hit_map.push(BouncedHitMap::default());
            }
        }
        self.hit_map[bounce].add_to_hitmap(hit_point, uuid, source)?;
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
    /// Returns the energy of the light that hit this surface: the energy of all its hits, of all
    /// bounces and bundles. The bundles of a bounce are summed in the order of their ids, so the
    /// sum does not depend on the order they are stored in.
    #[must_use]
    pub fn energy(&self) -> Energy {
        self.hit_map
            .iter()
            .flat_map(|bounced| bounced.hit_map.iter().sorted_by_key(|(uuid, _)| **uuid))
            .flat_map(|(_, rays)| rays.hit_points())
            .map(HitPoint::energy)
            .fold(joule!(0.0), |total, energy| total + energy)
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
    #[must_use]
    pub fn get_merged_rays_hit_map(&self) -> RaysHitMap {
        let mut merged_rays_hit_map = RaysHitMap::default();
        let mut count = 0;
        for bounced_hit_map in &self.hit_map {
            for hit_map in &bounced_hit_map.hit_map {
                if count == 0 {
                    merged_rays_hit_map = hit_map.1.clone();
                } else {
                    merged_rays_hit_map.merge(hit_map.1);
                }
                count += 1;
            }
        }
        merged_rays_hit_map
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
    pub fn get_first_hitpoints(&self) -> Option<&[HitPoint]> {
        for bhm in &self.hit_map {
            if bhm.hit_map.is_empty() {
                continue;
            }
            if let Some(rhm) = bhm.hit_map.values().next() {
                return Some(rhm.hit_points());
            }
        }
        None
    }
    /// Calculate a fluence map ([`FluenceData`]) of this [`HitMap`] that follows the ray tubes of
    /// its bundles, as the Voronoi estimator does.
    ///
    /// The tubes of all stored [`RaysHitMap`]s are drawn into one map, which covers all of them
    /// (see [`RaysHitMap::calc_fluence_with_tubes`]).
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    /// -`estimator`: the estimator the map stands for
    ///
    /// # Errors
    /// This function errors if the hits of a bundle do not all stand in the ray tubes of their
    /// source, or if no tube arrived complete.
    pub fn calc_combined_fluence_with_tubes(
        &self,
        nr_of_points: (usize, usize),
        estimator: FluenceEstimator,
    ) -> OpmResult<FluenceData> {
        let mut sheets = Vec::new();
        for rays_hit_map in self
            .hit_map
            .iter()
            .flat_map(|bounced| bounced.hit_map.values())
        {
            sheets.extend(rays_hit_map.tube_sheets()?);
        }
        draw_map(&sheets, nr_of_points, None, None, estimator)
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
    /// This function errors if [`RaysHitMap::calc_fluence_with_kde`] fails.
    pub fn calc_combined_fluence_with_kde(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        self.get_merged_rays_hit_map()
            .calc_fluence_with_kde(nr_of_points, None, None)
    }

    /// Calculate a fluence map ([`FluenceData`]) of this [`HitMap`] using the "Binning" method
    ///
    /// This method tries to combine the fluence of data of all stored [`RaysHitMap`]s and return a single [`FluenceData`]
    ///
    /// # Attributes
    /// -`nr_of_points`: tuple containing the number of (columns, rows) of the matrix on which the data should be calculated
    ///
    /// # Errors
    /// This function errors if [`RaysHitMap::calc_fluence_with_binning`] fails.
    pub fn calc_combined_fluence_with_binning(
        &self,
        nr_of_points: (usize, usize),
    ) -> OpmResult<FluenceData> {
        self.get_merged_rays_hit_map()
            .calc_fluence_with_binning(nr_of_points, None, None)
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
            FluenceEstimator::Voronoi => {
                self.calc_combined_fluence_with_tubes(nr_of_points, *estimator)
            }
            FluenceEstimator::KDE => self.calc_combined_fluence_with_kde(nr_of_points),
            FluenceEstimator::Binning => self.calc_combined_fluence_with_binning(nr_of_points),
        }
    }
}
impl From<HitMap> for Proptype {
    fn from(value: HitMap) -> Self {
        Self::HitMap(value)
    }
}

#[cfg(test)]
mod test_bounced_hit_map {
    use crate::{
        core_optics::hit_map::{BouncedHitMap, HitPoint},
        error::{OpmResult, OpossumError},
        joule, meter,
    };
    use uuid::Uuid;

    #[test]
    fn add_to_hitmap() -> OpmResult<()> {
        let mut bhm = BouncedHitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            uuid1,
            None,
        )?;
        assert_eq!(
            bhm.hit_map
                .get(&uuid1)
                .ok_or(OpossumError::Other("UUID1 not found in map".into()))?
                .hit_points()
                .len(),
            1
        );
        assert!(!bhm.hit_map.contains_key(&uuid2));
        bhm.add_to_hitmap(
            HitPoint::new(meter!(1.0, 0.0, 0.0), joule!(1.0))?,
            uuid1,
            None,
        )?;
        assert_eq!(
            bhm.hit_map
                .get(&uuid1)
                .ok_or(OpossumError::Other("UUID1 not found in map".into()))?
                .hit_points()
                .len(),
            2
        );
        assert!(!bhm.hit_map.contains_key(&uuid2));
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            uuid2,
            None,
        )?;
        assert_eq!(
            bhm.hit_map
                .get(&uuid1)
                .ok_or(OpossumError::Other("UUID1 not found in map".into()))?
                .hit_points()
                .len(),
            2
        );
        assert_eq!(
            bhm.hit_map
                .get(&uuid2)
                .ok_or(OpossumError::Other("UUID2 not found in map".into()))?
                .hit_points()
                .len(),
            1
        );
        Ok(())
    }
    #[test]
    fn get_rays_hit_map() -> OpmResult<()> {
        let mut bhm = BouncedHitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            uuid1,
            None,
        )?;
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            uuid1,
            None,
        )?;
        bhm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            uuid2,
            None,
        )?;
        assert_eq!(
            bhm.get_rays_hit_map(uuid1)
                .ok_or(OpossumError::Other("UUID1 not found in map".into()))?
                .hit_points()
                .len(),
            2
        );
        assert_eq!(
            bhm.get_rays_hit_map(uuid2)
                .ok_or(OpossumError::Other("UUID2 not found in map".into()))?
                .hit_points()
                .len(),
            1
        );
        assert!(bhm.get_rays_hit_map(Uuid::nil()).is_none());
        Ok(())
    }
}
#[cfg(test)]
mod test_hit_map {
    use approx::assert_relative_eq;
    use uuid::Uuid;

    use crate::{
        core_optics::hit_map::{HitMap, HitPoint, fluence_estimator::FluenceEstimator},
        error::OpmResult,
        joule,
        light::Ray,
        meter, nanometer,
        nodes::test_helper::helper::rays_along_z_from,
        properties::Proptype,
    };
    use nalgebra::Point3;
    use uom::si::f64::Length;

    #[test]
    fn hit_map() -> OpmResult<()> {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            uuid1,
            None,
        )?;
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            uuid2,
            None,
        )?;
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            0,
            uuid1,
            None,
        )?;
        assert_eq!(hm.hit_map().len(), 2);
        Ok(())
    }

    #[test]
    fn add_to_hitmap_energy() -> OpmResult<()> {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            Uuid::new_v4(),
            None,
        )?;
        assert_eq!(hm.hit_map.len(), 2);
        assert_eq!(hm.hit_map[0].hit_map.len(), 0);
        assert_eq!(hm.hit_map[1].hit_map.len(), 1);
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            0,
            Uuid::new_v4(),
            None,
        )?;
        assert_eq!(hm.hit_map.len(), 2);
        assert_eq!(hm.hit_map[0].hit_map.len(), 1);
        assert_eq!(hm.hit_map[1].hit_map.len(), 1);
        Ok(())
    }
    #[test]
    fn reset_energy() -> OpmResult<()> {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            Uuid::new_v4(),
            None,
        )?;
        hm.reset();
        assert!(hm.is_empty());
        Ok(())
    }
    #[test]
    fn is_empty() -> OpmResult<()> {
        let mut hm = HitMap::default();
        assert!(hm.is_empty());
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            Uuid::new_v4(),
            None,
        )?;
        assert!(!hm.is_empty());
        Ok(())
    }
    #[test]
    fn get_rays_hit_map() -> OpmResult<()> {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            uuid1,
            None,
        )?;
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            uuid2,
            None,
        )?;
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            0,
            uuid1,
            None,
        )?;
        assert!(hm.get_rays_hit_map(2, uuid1).is_none());
        assert!(hm.get_rays_hit_map(0, uuid2).is_none());
        Ok(())
    }
    #[test]
    fn get_merged_rays_hit_map() -> OpmResult<()> {
        let mut hm = HitMap::default();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            uuid1,
            None,
        )?;
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            1,
            uuid2,
            None,
        )?;
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.0, 0.0, 0.0), joule!(1.0))?,
            0,
            uuid1,
            None,
        )?;
        assert_eq!(hm.get_merged_rays_hit_map().hit_points().len(), 3);
        Ok(())
    }
    #[test]
    fn proptype_from() {
        let prop_type: Proptype = HitMap::default().into();
        assert!(matches!(prop_type, Proptype::HitMap(_)));
    }

    #[test]
    fn get_bounding_box() -> OpmResult<()> {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.1, 1.0, 0.0), joule!(1.0))?,
            1,
            Uuid::new_v4(),
            None,
        )?;
        hm.add_to_hitmap(
            HitPoint::new(meter!(-20.0, 20.0, 0.0), joule!(1.0))?,
            1,
            Uuid::new_v4(),
            None,
        )?;
        hm.add_to_hitmap(
            HitPoint::new(meter!(3.210, 0.0, 0.0), joule!(1.0))?,
            0,
            Uuid::new_v4(),
            None,
        )?;

        let (x_range, y_range) = hm.get_bounding_box();

        assert_eq!(x_range.start.value, -20.);
        assert_eq!(x_range.end.value, 3.21);
        assert_eq!(y_range.start.value, 0.0);
        assert_eq!(y_range.end.value, 20.);
        Ok(())
    }
    #[test]
    fn get_first_hitpoints() -> OpmResult<()> {
        let mut hm = HitMap::default();
        hm.add_to_hitmap(
            HitPoint::new(meter!(0.1, 1.0, 0.0), joule!(1.0))?,
            1,
            Uuid::new_v4(),
            None,
        )?;

        let hp = hm.get_first_hitpoints();
        assert!(hp.is_some());
        Ok(())
    }
    #[test]
    fn get_first_hitpoints_empty() {
        let hm = HitMap::default();
        let hp = hm.get_first_hitpoints();
        assert!(hp.is_none());
    }

    /// Adds hits at the given points, at bounce 1, of a bundle spanning the ray tubes between
    /// them, as a source spans them for a Voronoi map. `hit_of` makes the hit of each ray.
    fn add_bundle_in_tubes(
        hm: &mut HitMap,
        points: &[Point3<Length>],
        hit_of: fn(&Ray) -> OpmResult<HitPoint>,
    ) -> OpmResult<()> {
        let mut rays = rays_along_z_from(points, nanometer!(1000.0))?;
        rays.attach_mesh()?;
        let uuid = Uuid::new_v4();
        for ray in rays.iter() {
            hm.add_to_hitmap(hit_of(ray)?, 1, uuid, ray.source())?;
        }
        Ok(())
    }
    /// The hit of a ray where it starts, bringing its energy of 1 J.
    fn energy_hit(ray: &Ray) -> OpmResult<HitPoint> {
        HitPoint::new(ray.position(), ray.energy())
    }
    /// The corners and the center of a square of 1 m.
    fn square_with_center() -> Vec<Point3<Length>> {
        vec![
            meter!(-0.5, -0.5, 0.0),
            meter!(0., 0., 0.0),
            meter!(-0.5, 0.5, 0.0),
            meter!(0.5, 0.5, 0.0),
            meter!(0.5, -0.5, 0.0),
        ]
    }
    /// The corners of a diamond of 1 m² and its center.
    fn diamond_with_center() -> Vec<Point3<Length>> {
        vec![
            meter!(f64::sqrt(0.5), 0., 0.),
            meter!(0., f64::sqrt(0.5), 0.),
            meter!(-f64::sqrt(0.5), 0., 0.),
            meter!(0., -f64::sqrt(0.5), 0.),
            meter!(0., 0., 0.),
        ]
    }
    /// The center of the square stands for half of it, so its 1 J shows 2 J/m². A second bundle
    /// of the same kind adds as much again.
    #[test]
    fn a_combined_voronoi_map_adds_up_its_bundles() -> OpmResult<()> {
        let mut hm = HitMap::default();
        add_bundle_in_tubes(&mut hm, &square_with_center(), energy_hit)?;
        let fl_data = hm.calc_combined_fluence_with_tubes((51, 51), FluenceEstimator::Voronoi)?;
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 2.);
        add_bundle_in_tubes(&mut hm, &diamond_with_center(), energy_hit)?;
        let fl_data = hm.calc_combined_fluence_with_tubes((51, 51), FluenceEstimator::Voronoi)?;
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 4.);
        Ok(())
    }
    /// Hits that span no ray tubes leave no map.
    #[test]
    fn a_combined_map_of_hits_without_tubes_fails() -> OpmResult<()> {
        let mut hm = HitMap::default();
        let uuid = Uuid::new_v4();
        for pos in [meter!(-0.5, -0.5, 0.0), meter!(0., 0., 0.0)] {
            hm.add_to_hitmap(HitPoint::new(pos, joule!(1.0))?, 1, uuid, None)?;
        }
        for estimator in [FluenceEstimator::Voronoi] {
            assert!(
                hm.calc_combined_fluence_with_tubes((50, 50), estimator)
                    .is_err()
            );
        }
        Ok(())
    }
    #[test]
    fn calc_combined_fluence_with_kde() -> OpmResult<()> {
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
            hm.add_to_hitmap(HitPoint::new(*pos, joule!(1.0))?, 1, uuid, None)?;
        }
        let fl_data = hm.calc_combined_fluence_with_kde((51, 51))?;
        assert_relative_eq!(
            fl_data.interp_distribution()[(25, 25)].value,
            2.257264625439249
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
            hm.add_to_hitmap(HitPoint::new(*pos, joule!(1.0))?, 1, uuid, None)?;
        }
        let fl_data = hm.calc_combined_fluence_with_kde((51, 51))?;
        assert_relative_eq!(
            fl_data.interp_distribution()[(25, 25)].value,
            5.296802485042948
        );
        Ok(())
    }
    /// The 51 points of the map lie 1 m / 50 apart over the hits, so the 1 J in the middle stands
    /// for 0.0004 m²: 2500 J/m². The second bundle reaches √2 m wide and brings another 1 J to
    /// the middle, so 2 J over 0.0008 m² show the same.
    #[test]
    fn calc_combined_fluence_with_binning() -> OpmResult<()> {
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
            hm.add_to_hitmap(HitPoint::new(*pos, joule!(1.0))?, 1, uuid, None)?;
        }
        let fl_data = hm.calc_combined_fluence_with_binning((51, 51))?;
        assert_relative_eq!(fl_data.interp_distribution()[(25, 25)].value, 2500.0);

        let uuid = Uuid::new_v4();
        let pos2 = vec![
            meter!(f64::sqrt(0.5), 0., 0.),
            meter!(0., f64::sqrt(0.5), 0.),
            meter!(-f64::sqrt(0.5), 0., 0.),
            meter!(0., -f64::sqrt(0.5), 0.),
            meter!(0., 0., 0.),
        ];
        for pos in &pos2 {
            hm.add_to_hitmap(HitPoint::new(*pos, joule!(1.0))?, 1, uuid, None)?;
        }
        let fl_data = hm.calc_combined_fluence_with_binning((51, 51))?;
        assert_relative_eq!(
            fl_data.interp_distribution()[(25, 25)].value,
            2500.0,
            max_relative = 1e-12
        );
        Ok(())
    }
    #[test]
    fn calc_fluence_map() -> OpmResult<()> {
        let mut hm = HitMap::default();
        add_bundle_in_tubes(&mut hm, &square_with_center(), energy_hit)?;
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
        Ok(())
    }
}
