//! Module handling optical surfaces
use log::warn;
use uuid::Uuid;

use crate::{
    coatings::CoatingType,
    core_optics::{
        SurfaceConfig,
        hit_map::{
            HitMap,
            fluence_estimator::FluenceEstimator,
            rays_hit_map::{HitPoint, RaysHitMap},
        },
    },
    error::{OpmResult, OpossumError},
    geometry::Face,
    light::{RaySource, Rays},
    nodes::fluence_detector::Fluence,
    utils::geom_transformation::Isometry,
};
use core::fmt::Debug;

/// An optical surface of a component
///
/// One face of its geometry, with what the surface adds to it:
/// its [`CoatingType`], its laser induced damage threshold (LIDT), and what an analysis records
/// there (ray caches and a hit map).
///
/// The geometry holds one per face (see
/// [`GeometryKind::surface`](crate::geometry::GeometryKind::surface)) and traces rays at it (see
/// [`Geometry::interact`](crate::geometry::Geometry::interact)).
#[derive(Clone, PartialEq)]
pub struct OpticSurface {
    face: Face,
    coating: CoatingType,
    lidt: Fluence,
    backward_rays_cache: Vec<Rays>,
    forward_rays_cache: Vec<Rays>,
    hit_map: HitMap,
}
impl From<Face> for OpticSurface {
    /// The surface of a face with the default coating and LIDT (see [`SurfaceConfig::default`]).
    fn from(face: Face) -> Self {
        Self::new(face, &SurfaceConfig::default())
    }
}
impl OpticSurface {
    /// Creates a new [`OpticSurface`] with empty ray caches and an empty hit map.
    ///
    /// # Arguments
    ///
    /// * `face` - the face this surface is.
    /// * `config` - the coating and the LIDT of the surface.
    #[must_use]
    pub fn new(face: Face, config: &SurfaceConfig) -> Self {
        Self {
            face,
            coating: config.coating().clone(),
            lidt: config.lidt(),
            backward_rays_cache: Vec::new(),
            forward_rays_cache: Vec::new(),
            hit_map: HitMap::default(),
        }
    }
    /// Return the face this [`OpticSurface`] is.
    #[must_use]
    pub const fn face(&self) -> &Face {
        &self.face
    }
    /// Move the face of this [`OpticSurface`] by an isometry (see [`Face::move_by`]).
    pub fn move_by(&mut self, by: &Isometry) {
        self.face.move_by(by);
    }
    /// Set the coating and the LIDT of this [`OpticSurface`].
    pub fn set_config(&mut self, config: &SurfaceConfig) {
        self.coating = config.coating().clone();
        self.lidt = config.lidt();
    }
    /// Gets a reference to the forward / backward rays cache of this [`OpticSurface`].
    #[must_use]
    pub const fn get_rays_cache(&self, get_back_ward_cache: bool) -> &Vec<Rays> {
        if get_back_ward_cache {
            &self.backward_rays_cache
        } else {
            &self.forward_rays_cache
        }
    }
    /// Sets the coating of this [`OpticSurface`].
    pub fn set_coating(&mut self, coating: CoatingType) {
        self.coating = coating;
    }
    /// Returns a reference to the coating of this [`OpticSurface`].
    #[must_use]
    pub const fn coating(&self) -> &CoatingType {
        &self.coating
    }

    /// Sets the backwards rays cache of this [`OpticSurface`].
    pub fn set_backwards_rays_cache(&mut self, backward_rays_cache: Vec<Rays>) {
        self.backward_rays_cache = backward_rays_cache;
    }
    /// Sets the forward rays cache of this [`OpticSurface`].
    pub fn set_forward_rays_cache(&mut self, forward_rays_cache: Vec<Rays>) {
        self.forward_rays_cache = forward_rays_cache;
    }
    /// Adds a rays bundle to the rays cache of this [`OpticSurface`].
    pub fn add_to_rays_cache(&mut self, rays: Rays, add_to_forward_cache: bool) {
        if add_to_forward_cache {
            self.forward_rays_cache.push(rays);
        } else {
            self.backward_rays_cache.push(rays);
        }
    }
    /// Returns a reference to the hit map of this [`OpticSurface`].
    ///
    /// This function returns a vector of intersection points (with energies) of [`Rays`] that hit the surface.
    #[must_use]
    pub const fn hit_map(&self) -> &HitMap {
        &self.hit_map
    }
    ///returns a reference to a [`RaysHitMap`] in this [`OpticSurface`]
    #[must_use]
    pub fn get_rays_hit_map(&self, bounce: usize, uuid: Uuid) -> Option<&RaysHitMap> {
        self.hit_map.get_rays_hit_map(bounce, uuid)
    }
    /// Add intersection point (with energy) to hit map.
    ///
    /// # Arguments
    ///
    /// * `hit_point` - where the ray hit and what it brought.
    /// * `bounce` - the number of bounces of the ray.
    /// * `rays_uuid` - the bundle of the ray.
    /// * `source` - where the ray stands in the ray tubes of its source, if it does.
    ///
    /// # Errors
    /// This function errors if adding the hit point to the hit map fails
    pub fn add_to_hit_map(
        &mut self,
        hit_point: HitPoint,
        bounce: usize,
        rays_uuid: Uuid,
        source: Option<&RaySource>,
    ) -> OpmResult<()> {
        self.hit_map
            .add_to_hitmap(hit_point, bounce, rays_uuid, source)
    }
    /// Reset hit map of this [`OpticSurface`].
    pub fn reset_hit_map(&mut self) {
        self.hit_map.reset();
    }

    /// Evaluate the fluence of a given ray bundle on this surface: its fluence map of 101 × 101
    /// points. If the peak of the map surpasses the lidt, store the map with the critical fluence
    /// parameters in the hitmap.
    ///
    /// # Errors
    ///
    /// This function does not fail: a map that cannot be computed is warned about, and the
    /// bundle is left out.
    pub fn evaluate_fluence_of_ray_bundle(
        &mut self,
        rays: &Rays,
        estimator: &FluenceEstimator,
    ) -> OpmResult<()> {
        if let Some(rays_hit_map) = self.get_rays_hit_map(rays.bounce_lvl(), rays.uuid()) {
            match rays_hit_map.calc_fluence_map((101, 101), estimator, None, None) {
                Ok(map) if map.peak() > self.lidt => {
                    self.hit_map
                        .add_critical_fluence(rays.uuid(), rays.ray_history_len(), map);
                }
                Ok(_) => {}
                Err(error) => warn!(
                    "Could not estimate maximum fluence of ray bundle! Ray bundle will be ignored during calculation: {error}"
                ),
            }
        }
        Ok(())
    }
    ///returns a reference to the lidt value of this [`OpticSurface`]
    #[must_use]
    pub fn lidt(&self) -> &Fluence {
        &self.lidt
    }
    /// Sets the laser induced damage threshold (LIDT) of this [`OpticSurface`]
    ///
    /// # Errors
    ///
    /// This function returns an error if the given LIDT is negative or NaN.
    pub fn set_lidt(&mut self, lidt: Fluence) -> OpmResult<()> {
        if lidt.is_sign_negative() || lidt.is_nan() {
            return Err(OpossumError::Other(
                "LIDT must be positive and not NaN".into(),
            ));
        }
        self.lidt = lidt;
        Ok(())
    }
}

impl Debug for OpticSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpticSurface")
            .field("face", &self.face)
            .field("coating", &self.coating)
            .field("lidt", &self.lidt)
            .finish_non_exhaustive()
    }
}
#[cfg(test)]
mod test {
    use super::{Face, FluenceEstimator, HitPoint, Isometry, OpticSurface, SurfaceConfig};
    use crate::{
        J_per_cm2,
        coatings::CoatingType,
        error::{OpmResult, OpossumError},
        geometry::SurfaceShape,
        joule,
        light::{Ray, Rays},
        meter, nanometer,
        nodes::test_helper::helper::rays_along_z_from,
    };

    /// A plane through the origin.
    fn plane() -> Face {
        Face::new(SurfaceShape::Plane, Isometry::identity())
    }
    /// The surface of a face alone has the default coating and LIDT and records nothing yet.
    #[test]
    fn the_surface_of_a_face_has_the_default_configuration() {
        let os = OpticSurface::from(plane());
        assert_eq!(os.face(), &plane());
        assert!(matches!(os.coating, CoatingType::IdealAR));
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
        assert!(os.hit_map.is_empty());
        assert_eq!(os.lidt, J_per_cm2!(f64::INFINITY));
    }
    #[test]
    fn a_surface_carries_its_configuration() -> OpmResult<()> {
        let config = SurfaceConfig::new(CoatingType::Fresnel, J_per_cm2!(2.0))?;
        let mut os = OpticSurface::new(plane(), &config);
        assert_eq!(os.lidt, J_per_cm2!(2.0));
        assert!(matches!(os.coating, CoatingType::Fresnel));
        os.set_config(&SurfaceConfig::default());
        assert_eq!(os.lidt, J_per_cm2!(f64::INFINITY));
        assert!(matches!(os.coating, CoatingType::IdealAR));
        Ok(())
    }
    #[test]
    fn set_lidt() {
        let mut os = OpticSurface::from(plane());
        assert!(os.set_lidt(J_per_cm2!(f64::NAN)).is_err());
        assert!(os.set_lidt(J_per_cm2!(f64::NEG_INFINITY)).is_err());
        assert!(os.set_lidt(J_per_cm2!(-0.1)).is_err());
        assert!(os.set_lidt(J_per_cm2!(f64::INFINITY)).is_ok());
        assert!(os.set_lidt(J_per_cm2!(2.5)).is_ok());

        assert_eq!(os.lidt, J_per_cm2!(2.5));
        assert_eq!(*os.lidt(), J_per_cm2!(2.5));
    }
    #[test]
    fn add_to_rays_cache() -> OpmResult<()> {
        let mut os = OpticSurface::from(plane());
        let rays = Rays::from(Ray::new_collimated(
            meter!(0.0, 0.0, 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        os.add_to_rays_cache(rays.clone(), true);
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 1);
        os.add_to_rays_cache(rays.clone(), false);
        assert_eq!(os.backward_rays_cache.len(), 1);
        assert_eq!(os.forward_rays_cache.len(), 1);
        Ok(())
    }
    #[test]
    fn set_backwards_rays_cache() -> OpmResult<()> {
        let mut os = OpticSurface::from(plane());
        let rays = Rays::from(Ray::new_collimated(
            meter!(0.0, 0.0, 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        os.set_backwards_rays_cache(vec![rays]);
        assert_eq!(os.backward_rays_cache.len(), 1);
        assert_eq!(os.forward_rays_cache.len(), 0);
        os.set_backwards_rays_cache(vec![]);
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
        Ok(())
    }
    #[test]
    fn set_forwards_rays_cache() -> OpmResult<()> {
        let mut os = OpticSurface::from(plane());
        let rays = Rays::from(Ray::new_collimated(
            meter!(0.0, 0.0, 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        os.set_forward_rays_cache(vec![rays]);
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 1);
        os.set_forward_rays_cache(vec![]);
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
        Ok(())
    }
    /// A bundle whose map peaks above the LIDT of the surface leaves its map there as a critical
    /// fluence; below the LIDT it leaves nothing.
    #[test]
    fn a_bundle_above_the_lidt_leaves_its_map() -> OpmResult<()> {
        let mut os = OpticSurface::from(plane());
        let rays = rays_along_z_from(
            &[
                meter!(0.0, 0.0, 0.0),
                meter!(1.0, 0.0, 0.0),
                meter!(0.0, 1.0, 0.0),
                meter!(1.0, 1.0, 0.0),
            ],
            nanometer!(1000.0),
        )?;
        for ray in &rays {
            let hit = HitPoint::new(ray.position(), ray.energy())?;
            os.add_to_hit_map(hit, rays.bounce_lvl(), rays.uuid(), None)?;
        }
        os.evaluate_fluence_of_ray_bundle(&rays, &FluenceEstimator::Binning)?;
        assert!(os.hit_map().critical_fluences().is_empty());
        os.set_lidt(J_per_cm2!(1e-9))?;
        os.evaluate_fluence_of_ray_bundle(&rays, &FluenceEstimator::Binning)?;
        let (map, history_position) = os
            .hit_map()
            .critical_fluences()
            .get(&rays.uuid())
            .ok_or_else(|| OpossumError::Other("no critical fluence".into()))?;
        assert!(map.peak() > J_per_cm2!(1e-9));
        assert_eq!(*map.estimator(), FluenceEstimator::Binning);
        assert_eq!(*history_position, rays.ray_history_len());
        Ok(())
    }
    #[test]
    fn get_rays_cache() -> OpmResult<()> {
        let mut os = OpticSurface::from(plane());
        let rays = Rays::from(Ray::new_collimated(
            meter!(0.0, 0.0, 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        os.add_to_rays_cache(rays.clone(), true);
        assert_eq!(os.get_rays_cache(true).len(), 0);
        assert_eq!(os.get_rays_cache(false).len(), 1);
        Ok(())
    }
}
