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
    light::Rays,
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
    /// Stores a critical fluence in a hitmap
    fn add_critical_fluence(
        &mut self,
        uuid: Uuid,
        rays_hist_pos: usize,
        fluence: Fluence,
        bounce: usize,
    ) {
        self.hit_map
            .add_critical_fluence(uuid, rays_hist_pos, fluence, bounce);
    }

    ///returns a reference to a [`RaysHitMap`] in this [`OpticSurface`]
    #[must_use]
    pub fn get_rays_hit_map(&self, bounce: usize, uuid: Uuid) -> Option<&RaysHitMap> {
        self.hit_map.get_rays_hit_map(bounce, uuid)
    }
    /// Add intersection point (with energy) to hit map.
    ///
    /// # Errors
    /// This function errors if adding the hit point to the hit map fails
    pub fn add_to_hit_map(
        &mut self,
        hit_point: HitPoint,
        bounce: usize,
        rays_uuid: Uuid,
    ) -> OpmResult<()> {
        self.hit_map.add_to_hitmap(hit_point, bounce, rays_uuid)
    }
    /// Reset hit map of this [`OpticSurface`].
    pub fn reset_hit_map(&mut self) {
        self.hit_map.reset();
    }

    /// Evaluate the fluence of a given ray bundle on this surface. If the fluence
    /// surpasses its lidt, store the critical fluence parameters in the hitmap
    ///
    /// # Errors
    ///
    /// This function errors on error propagation of `calc_fluence`
    pub fn evaluate_fluence_of_ray_bundle(
        &mut self,
        rays: &Rays,
        estimator: &FluenceEstimator,
    ) -> OpmResult<()> {
        if let Some(rays_hit_map) = self.get_rays_hit_map(rays.bounce_lvl(), rays.uuid()) {
            if let Ok(peak_fluence) = rays_hit_map.get_max_fluence(estimator) {
                if peak_fluence > self.lidt {
                    self.add_critical_fluence(
                        rays.uuid(),
                        rays.ray_history_len(),
                        peak_fluence,
                        rays.bounce_lvl(),
                    );
                }
            } else {
                warn!(
                    "Could not estimate maximum fluence of ray bundle! Ray bundle will be ignored during calculation"
                );
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
    use super::{Face, Isometry, OpticSurface, SurfaceConfig};
    use crate::{
        J_per_cm2,
        coatings::CoatingType,
        error::{OpmResult, OpossumError},
        geometry::SurfaceShape,
        joule,
        light::{Ray, Rays},
        meter, nanometer,
    };
    use uuid::Uuid;

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
    #[test]
    fn add_critical_fluence() -> OpmResult<()> {
        let mut os = OpticSurface::from(plane());
        let uuid = Uuid::new_v4();
        os.add_critical_fluence(uuid, 1, J_per_cm2!(1.0), 2);
        let hit_map = os.hit_map();
        assert!(hit_map.critical_fluences().get(&Uuid::nil()).is_none());
        let critical_fluence = hit_map
            .critical_fluences()
            .get(&uuid)
            .ok_or(OpossumError::Other("Error getting critical fluence".into()))?;
        assert_eq!(critical_fluence.0, J_per_cm2!(1.0));
        assert_eq!(critical_fluence.1, 1);
        assert_eq!(critical_fluence.2, 2);
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
