//! Module handling optical surfaces
use log::warn;
use uuid::Uuid;

use crate::{
    J_per_cm2,
    analyzers::propagation_strategy::PropagationStrategy,
    coatings::CoatingType,
    core_optics::hit_map::{
        HitMap,
        fluence_estimator::FluenceEstimator,
        rays_hit_map::{HitPoint, RaysHitMap},
    },
    error::{OpmResult, OpossumError},
    geometry::Geometry,
    light::Rays,
    nodes::fluence_detector::Fluence,
    refractive_index::RefractiveIndexType,
};
use core::fmt::Debug;

/// An optical surface of a node: one face of the node's geometry, with what the surface adds to it,
/// such as its [`CoatingType`].
///
/// It holds no geometry. Where a ray hits the surface is asked of the geometry the node keeps (see
/// [`Geometry::intersect`]), with the number of the face this surface is.
#[derive(Clone)]
pub struct OpticSurface {
    face: usize,
    coating: CoatingType,
    lidt: Fluence,
    backward_rays_cache: Vec<Rays>,
    forward_rays_cache: Vec<Rays>,
    hit_map: HitMap,
}
impl Default for OpticSurface {
    /// Returns a default [`OpticSurface`].
    ///
    /// The default is face 0 with an ideal antireflective coating (=no reflection) and a lidt of
    /// 1 J/cm².
    fn default() -> Self {
        Self {
            face: 0,
            coating: CoatingType::IdealAR,
            lidt: J_per_cm2!(1.),
            backward_rays_cache: Vec::<Rays>::new(),
            forward_rays_cache: Vec::<Rays>::new(),
            hit_map: HitMap::default(),
        }
    }
}
impl OpticSurface {
    /// Creates a new [`OpticSurface`].
    ///
    /// **Note**: The laser induced damage threshold (LIDT) can be set to infinity to model an
    /// "unbreakable" optical surface.
    ///
    /// # Arguments
    ///
    /// * `face` - the number of the face of the node's geometry this surface is.
    /// * `coating` - the coating of the surface.
    /// * `lidt` - the laser induced damage threshold of the surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given lidt is negative or NaN.
    pub fn new(face: usize, coating: CoatingType, lidt: Fluence) -> OpmResult<Self> {
        if lidt.is_sign_negative() || lidt.is_nan() {
            return Err(OpossumError::Other(
                "LIDT must be positive and not NaN".into(),
            ));
        }
        Ok(Self {
            face,
            coating,
            lidt,
            ..Default::default()
        })
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
    /// Returns the number of the face of the node's geometry this [`OpticSurface`] is.
    #[must_use]
    pub const fn face(&self) -> usize {
        self.face
    }
    /// Sets the coating of this [`OpticSurface`].
    pub const fn set_coating(&mut self, coating: CoatingType) {
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

    /// Propagates a bundle of rays through this specific surface.
    /// Utilizes the Strategy Pattern to execute analyzer-specific behavior.
    ///
    /// # Parameters
    /// * `geometry`: The geometry of the node this surface is a face of, which answers where the
    ///   rays hit it.
    /// * `rays_bundle`: The bundle of rays to be propagated.
    /// * `node_uuid`: The UUID of the parent optical node (used for setting the origin of reflected rays).
    /// * `refri_after_surf`: The refractive index after the surface.
    /// * `backward`: The direction of propagation (`true` if backwards).
    /// * `refraction_intended`: Indicates whether refraction should be calculated.
    /// * `strategy`: The strategy defining the behavior of the current analysis mode.
    ///
    /// # Returns
    /// For every bundle in `rays_bundle` after the call, whether each of its rays hit this surface
    /// (see [`Rays::refract_on_surface`]). The ghost reflections appended from the cache of this
    /// surface left it from a hit, so all of them count as hits.
    ///
    /// # Errors
    /// This function errors if the optical calculations (refraction/reflection) or the
    /// strategy-specific hooks (e.g., fluence evaluation) fail.
    #[allow(clippy::too_many_arguments)]
    pub fn propagate_rays(
        &mut self,
        geometry: &Geometry,
        rays_bundle: &mut Vec<Rays>,
        node_uuid: Uuid,
        refri_after_surf: Option<&RefractiveIndexType>,
        backward: bool,
        refraction_intended: bool,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<Vec<Vec<bool>>> {
        let missed_strategy = strategy.missed_surface_strategy();
        let mut hits_per_bundle = Vec::with_capacity(rays_bundle.len());
        for rays in &mut *rays_bundle {
            let (mut reflected, hits) = rays.refract_on_surface(
                geometry,
                self,
                refri_after_surf,
                refraction_intended,
                &missed_strategy,
            )?;
            reflected.set_node_origin_uuid(node_uuid);
            strategy.on_surface_interaction(self, rays, reflected, backward)?;
            hits_per_bundle.push(hits);
        }
        for rays in self.get_rays_cache(backward) {
            hits_per_bundle.push(vec![true; rays.nr_of_rays(false)]);
            rays_bundle.push(rays.clone());
        }
        Ok(hits_per_bundle)
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
    use super::OpticSurface;
    use crate::{
        J_per_cm2,
        analyzers::RayTraceConfig,
        apertures::{CircleShape, Outline},
        coatings::CoatingType,
        error::{OpmResult, OpossumError},
        geometry::{Extruded, Geometry},
        joule,
        light::{Ray, Rays},
        meter, millimeter, nanometer,
    };
    use uuid::Uuid;

    /// A plane through the node, its component reaching 5 mm around the node's axis.
    fn bounded_plane() -> OpmResult<Geometry> {
        Ok(Geometry::plane(Some(Outline::new(
            CircleShape::new(millimeter!(5.0))?,
            Vec::new(),
        )?)))
    }
    /// A ray along z, the given distance off the axis in y.
    fn ray_at(y: f64) -> OpmResult<Ray> {
        Ray::new_collimated(millimeter!(0.0, y, -10.0), nanometer!(1000.0), joule!(1.0))
    }
    /// The surface pass tells for each ray of each bundle whether it hit. Ghost reflections cached
    /// at the surface left it from a hit, so when they are appended they count as hits.
    #[test]
    fn propagation_tells_which_rays_hit() -> OpmResult<()> {
        let mut surface = OpticSurface::default();
        surface.set_forward_rays_cache(vec![Rays::from(vec![ray_at(0.0)?, ray_at(7.0)?])]);
        let mut bundles = vec![Rays::from(vec![ray_at(4.9)?, ray_at(5.1)?])];
        let hits = surface.propagate_rays(
            &bounded_plane()?,
            &mut bundles,
            Uuid::new_v4(),
            None,
            false,
            true,
            &RayTraceConfig::default(),
        )?;
        assert_eq!(hits, [vec![true, false], vec![true, true]]);
        assert_eq!(bundles.len(), 2);
        Ok(())
    }
    /// A ray trace invalidates the rays the surface pass leaves below its energy threshold. The
    /// positioning run keeps them: geometry alone places the optical axis.
    #[test]
    fn a_ray_trace_drops_rays_below_the_energy_threshold() -> OpmResult<()> {
        let mut config = RayTraceConfig::default();
        config.set_min_energy_per_ray(joule!(0.5))?;
        for (config, valid) in [(config.for_positioning(), 2), (config, 1)] {
            let weak = Ray::new_collimated(
                millimeter!(0.0, 0.0, -10.0),
                nanometer!(1000.0),
                joule!(0.1),
            )?;
            let mut bundles = vec![Rays::from(vec![ray_at(0.0)?, weak])];
            OpticSurface::default().propagate_rays(
                &bounded_plane()?,
                &mut bundles,
                Uuid::new_v4(),
                None,
                false,
                true,
                &config,
            )?;
            assert_eq!(bundles[0].nr_of_rays(true), valid);
        }
        Ok(())
    }
    /// A surface is traced at the face of the geometry it names: the rear face of a plate here, so
    /// a ray along the axis meets it 10 mm behind the front.
    #[test]
    fn a_surface_is_traced_at_the_face_it_names() -> OpmResult<()> {
        let plate = Geometry::singlet(
            millimeter!(f64::INFINITY),
            millimeter!(f64::INFINITY),
            millimeter!(10.0),
            Outline::new(CircleShape::new(millimeter!(5.0))?, Vec::new())?,
        )?;
        let mut surface = OpticSurface::new(Extruded::REAR, CoatingType::IdealAR, J_per_cm2!(1.0))?;
        assert_eq!(surface.face(), Extruded::REAR);
        let mut bundles = vec![Rays::from(ray_at(0.0)?)];
        surface.propagate_rays(
            &plate,
            &mut bundles,
            Uuid::new_v4(),
            None,
            false,
            true,
            &RayTraceConfig::default(),
        )?;
        let ray = bundles[0]
            .iter()
            .next()
            .ok_or_else(|| OpossumError::Other("the bundle lost its ray".into()))?;
        assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
        Ok(())
    }

    #[test]
    fn default() {
        let os = OpticSurface::default();
        assert_eq!(os.face(), 0);
        assert!(matches!(os.coating, CoatingType::IdealAR));
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
        assert!(os.hit_map.is_empty());
        assert_eq!(os.lidt, J_per_cm2!(1.0));
    }
    #[test]
    fn new() -> OpmResult<()> {
        for lidt in [f64::NAN, f64::NEG_INFINITY, -0.1] {
            assert!(OpticSurface::new(0, CoatingType::IdealAR, J_per_cm2!(lidt)).is_err());
        }
        assert!(OpticSurface::new(0, CoatingType::IdealAR, J_per_cm2!(f64::INFINITY)).is_ok());
        let os = OpticSurface::new(1, CoatingType::Fresnel, J_per_cm2!(2.0))?;
        assert_eq!(os.face(), 1);
        assert_eq!(os.lidt, J_per_cm2!(2.0));
        assert!(matches!(os.coating, CoatingType::Fresnel));
        Ok(())
    }
    #[test]
    fn set_lidt() {
        let mut os = OpticSurface::default();
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
        let mut os = OpticSurface::default();
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
        let mut os = OpticSurface::default();
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
        let mut os = OpticSurface::default();
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
        let mut os = OpticSurface::default();
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
        let mut os = OpticSurface::default();
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
