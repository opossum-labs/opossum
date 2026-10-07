//! Module handling optical surfaces
use log::warn;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    J_per_cm2,
    analyzers::propagation_strategy::PropagationStrategy,
    apertures::Aperture,
    coatings::CoatingType,
    core_optics::hit_map::{
        HitMap,
        fluence_estimator::FluenceEstimator,
        rays_hit_map::{HitPoint, RaysHitMap},
    },
    error::{OpmResult, OpossumError},
    geometry::{Rim, geo_surface::GeoSurfaceRef},
    light::{Ray, Rays},
    nodes::fluence_detector::Fluence,
    refractive_index::RefractiveIndexType,
    utils::{LockExt, geom_transformation::Isometry},
};
use core::fmt::Debug;
use nalgebra::{Point3, Vector3};
use uom::si::f64::Length;

/// This struct represents an optical surface, which consists of the geometric surface shape
/// ([`GeoSurface`](crate::geometry::geo_surface::GeoSurface)) and further properties such as the [`CoatingType`].
#[derive(Serialize, Deserialize, Clone)]
pub struct OpticSurface {
    #[serde(skip)]
    geo_surface: GeoSurfaceRef,
    anchor_point_iso: Isometry,
    #[serde(skip)]
    rim: Option<Rim>,
    #[serde(skip_serializing_if = "Aperture::is_none", default)]
    aperture: Aperture,
    coating: CoatingType,
    lidt: Fluence,
    #[serde(skip)]
    backward_rays_cache: Vec<Rays>,
    #[serde(skip)]
    forward_rays_cache: Vec<Rays>,
    #[serde(skip)]
    hit_map: HitMap,
}
impl Default for OpticSurface {
    /// Returns a default [`OpticSurface`].
    ///
    /// The default is a flat surface with an ideal antireflective coating (=no reflection), no limiting aperture
    /// and a lidt of 1 J/cm².
    fn default() -> Self {
        Self {
            geo_surface: GeoSurfaceRef::default(),
            anchor_point_iso: Isometry::identity(),
            rim: None,
            aperture: Aperture::default(),
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
    /// # Errors
    ///
    /// This function returns an error if the given lidt is negative or NaN.
    pub fn new(
        geo_surface: GeoSurfaceRef,
        coating: CoatingType,
        aperture: Aperture,
        lidt: Fluence,
    ) -> OpmResult<Self> {
        if lidt.is_sign_negative() || lidt.is_nan() {
            return Err(OpossumError::Other(
                "LIDT must be positive and not NaN".into(),
            ));
        }
        Ok(Self {
            geo_surface,
            aperture,
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
    /// Sets the geo surface of this [`OpticSurface`].
    pub fn set_geo_surface(&mut self, geo_surface: GeoSurfaceRef) {
        self.geo_surface = geo_surface;
    }

    /// Sets the aperture of this [`OpticSurface`].
    pub fn set_aperture(&mut self, aperture: Aperture) {
        self.aperture = aperture;
    }
    /// Sets the coating of this [`OpticSurface`].
    pub const fn set_coating(&mut self, coating: CoatingType) {
        self.coating = coating;
    }
    /// Returns a reference to the geo surface of this [`OpticSurface`].
    #[must_use]
    pub fn geo_surface(&self) -> GeoSurfaceRef {
        self.geo_surface.clone()
    }
    /// Sets the [`Rim`] of this [`OpticSurface`]: the lateral boundary of its component, stated
    /// relative to the node. `None` leaves the surface unbounded.
    pub fn set_rim(&mut self, rim: Option<Rim>) {
        self.rim = rim;
    }
    /// Returns the [`Rim`] of this [`OpticSurface`], or `None` if it is unbounded.
    #[must_use]
    pub const fn rim(&self) -> Option<&Rim> {
        self.rim.as_ref()
    }
    /// Intersect a [`Ray`] with this [`OpticSurface`].
    ///
    /// This is the one place that decides whether, and where, a ray hits this surface. A point
    /// outside the surface's [`Rim`] is not part of the component, so a ray reaching the surface
    /// only there misses it.
    ///
    /// The rim follows the geometric surface: its frame is derived from where that surface sits
    /// right now (its isometry, minus its anchor, is the node's frame).
    ///
    /// # Arguments
    ///
    /// * `ray` - the ray to be intersected.
    ///
    /// # Returns
    ///
    /// The intersection point in global coordinates and the normalized surface normal there,
    /// pointing against the ray; or `None` if the ray misses the surface or meets it outside its
    /// rim.
    ///
    /// # Errors
    ///
    /// This function returns an error if the mutex of the geometric surface cannot be locked.
    pub fn intersect(&self, ray: &Ray) -> OpmResult<Option<(Point3<Length>, Vector3<f64>)>> {
        let geo_surface = self.geo_surface.0.lock_opm()?;
        let intersection = geo_surface.calc_intersect_and_normal(ray);
        let Some(rim) = &self.rim else {
            return Ok(intersection);
        };
        let node_frame = Isometry::new_from_transform(
            geo_surface.isometry().get_transform() * self.anchor_point_iso.get_inv_transform(),
        );
        Ok(intersection.filter(|(point, _)| rim.contains(point, &node_frame)))
    }
    /// Returns a reference to the aperture of this [`OpticSurface`].
    #[must_use]
    pub const fn aperture(&self) -> &Aperture {
        &self.aperture
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
    /// Sets the isometry of this [`OpticSurface`].
    ///
    /// # Panics
    ///
    /// This function might theoretically panic if locking of an internal mutex fails.
    pub fn set_isometry(&self, iso: Isometry) {
        self.geo_surface.0.lock_opm().unwrap().set_isometry(iso);
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

    /// Prunes the hit map of this [`OpticSurface`] by removing all points that are outside the aperture.
    pub fn prune_hit_map(&mut self, iso: &Isometry) {
        if !self.aperture.is_none() {
            self.hit_map.prune_by_aperture(&self.aperture, iso);
        }
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

    /// Sets the anchor point isometry of this [`OpticSurface`]
    pub const fn set_anchor_point_iso(&mut self, iso: Isometry) {
        self.anchor_point_iso = iso;
    }

    ///Returns a reference to the anchor point isometry of this [`OpticSurface`]
    #[must_use]
    pub const fn anchor_point_iso(&self) -> &Isometry {
        &self.anchor_point_iso
    }
    /// Propagates a bundle of rays through this specific surface.
    /// Utilizes the Strategy Pattern to execute analyzer-specific behavior.
    ///
    /// # Parameters
    /// * `rays_bundle`: The bundle of rays to be propagated.
    /// * `node_uuid`: The UUID of the parent optical node (used for setting the origin of reflected rays).
    /// * `iso`: The effective isometry of this surface in 3D space.
    /// * `refri_after_surf`: The refractive index after the surface.
    /// * `backward`: The direction of propagation (`true` if backwards).
    /// * `refraction_intended`: Indicates whether refraction should be calculated.
    /// * `strategy`: The strategy defining the behavior of the current analysis mode.
    ///
    /// # Errors
    /// This function errors if the optical calculations (refraction/reflection) or the
    /// strategy-specific hooks (e.g., fluence evaluation) fail.
    #[allow(clippy::too_many_arguments)]
    pub fn propagate_rays(
        &mut self,
        rays_bundle: &mut Vec<Rays>,
        node_uuid: Uuid,
        iso: &Isometry,
        refri_after_surf: Option<&RefractiveIndexType>,
        backward: bool,
        refraction_intended: bool,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        let missed_strategy = strategy.missed_surface_strategy();

        for rays in &mut *rays_bundle {
            let (mut reflected, hits) = rays.refract_on_surface(
                self,
                refri_after_surf,
                refraction_intended,
                &missed_strategy,
            )?;
            reflected.set_node_origin_uuid(node_uuid);
            strategy.on_surface_interaction(self, rays, reflected, backward)?;
            // The port aperture masks light that passed this surface, not rays that ran past it.
            rays.apodize_hits(self.aperture(), iso, &hits)?;
            strategy.on_after_apodization(rays)?;
        }
        for rays in self.get_rays_cache(backward) {
            rays_bundle.push(rays.clone());
        }
        self.prune_hit_map(iso);
        Ok(())
    }
}

impl Debug for OpticSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("OpticSurface");
        ds.field("aperture", &self.aperture);
        ds.field("coating", &self.coating);

        // Try to lock, if it fails, provide a descriptive string instead
        match self.geo_surface.0.lock_opm() {
            Ok(guard) => ds.field("geometric surface", &*guard),
            Err(_) => ds.field("geometric surface", &"<Locked/Poisoned>"),
        };

        ds.field("lidt", &self.lidt);
        ds.finish_non_exhaustive()
    }
}
#[cfg(test)]
mod test {
    use super::OpticSurface;
    use crate::{
        J_per_cm2,
        apertures::{Aperture, ApertureShape, ApertureType, CircleShape},
        coatings::CoatingType,
        degree,
        error::{OpmResult, OpossumError},
        geometry::{Plane, Rim, Sphere, geo_surface::GeoSurfaceRef},
        joule,
        light::{Ray, Rays},
        meter, millimeter, nanometer,
        types::validated_type_definitions::ValidatedCrossSection,
        utils::geom_transformation::Isometry,
    };
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    /// A plane placed at the given anchor of a node at the origin, its component reaching 5 mm
    /// around the node's axis.
    fn bounded_plane(anchor: Isometry) -> OpmResult<OpticSurface> {
        let mut surface = OpticSurface::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(anchor)))),
            CoatingType::IdealAR,
            Aperture::default(),
            J_per_cm2!(1.0),
        )?;
        surface.set_anchor_point_iso(anchor);
        surface.set_rim(Some(Rim::new(
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(5.0),
                ApertureType::Hole,
                None,
            )?)?,
            Isometry::identity(),
        )));
        Ok(surface)
    }
    /// A ray along z, the given distance off the axis in y.
    fn ray_at(y: f64) -> OpmResult<Ray> {
        Ray::new_collimated(millimeter!(0.0, y, -10.0), nanometer!(1000.0), joule!(1.0))
    }
    #[test]
    fn intersect_misses_outside_the_rim() -> OpmResult<()> {
        let surface = bounded_plane(Isometry::identity())?;
        assert!(surface.intersect(&ray_at(4.9)?)?.is_some());
        assert!(surface.intersect(&ray_at(5.1)?)?.is_none());
        Ok(())
    }
    #[test]
    fn the_rim_is_measured_in_the_node_frame() -> OpmResult<()> {
        // Tilted by 30°, the plane is hit at y = 4.9 mm by a ray along the node's axis. In the plane's
        // own frame that point lies 4.9 / cos 30° = 5.66 mm out, beyond the 5 mm rim.
        let surface = bounded_plane(Isometry::new(
            millimeter!(0.0, 0.0, 0.0),
            degree!(30.0, 0.0, 0.0),
        )?)?;
        assert!(surface.intersect(&ray_at(4.9)?)?.is_some());
        assert!(surface.intersect(&ray_at(5.1)?)?.is_none());
        Ok(())
    }
    #[test]
    fn the_rim_follows_its_surface() -> OpmResult<()> {
        // Moved without being installed anew, the surface takes its rim along.
        let surface = bounded_plane(Isometry::identity())?;
        surface.set_isometry(Isometry::new_translation(millimeter!(0.0, 20.0, 0.0))?);
        assert!(surface.intersect(&ray_at(0.0)?)?.is_none());
        assert!(surface.intersect(&ray_at(20.0)?)?.is_some());
        Ok(())
    }

    #[test]
    fn default() {
        let os = OpticSurface::default();
        assert!(matches!(os.aperture.shape(), ApertureShape::Open));
        assert!(matches!(os.coating, CoatingType::IdealAR));
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
        assert!(os.hit_map.is_empty());
        assert_eq!(os.lidt, J_per_cm2!(1.0));
    }
    #[test]
    fn new() -> OpmResult<()> {
        let gs = GeoSurfaceRef::default();
        assert!(
            OpticSurface::new(
                gs.clone(),
                CoatingType::IdealAR,
                Aperture::default(),
                J_per_cm2!(f64::NAN)
            )
            .is_err()
        );
        assert!(
            OpticSurface::new(
                gs.clone(),
                CoatingType::IdealAR,
                Aperture::default(),
                J_per_cm2!(f64::NEG_INFINITY)
            )
            .is_err()
        );
        assert!(
            OpticSurface::new(
                gs.clone(),
                CoatingType::IdealAR,
                Aperture::default(),
                J_per_cm2!(-0.1)
            )
            .is_err()
        );
        assert!(
            OpticSurface::new(
                gs.clone(),
                CoatingType::IdealAR,
                Aperture::default(),
                J_per_cm2!(f64::INFINITY)
            )
            .is_ok()
        );

        let aperture = Aperture::new(
            ApertureShape::BinaryCircle(CircleShape::new(meter!(1.0))?),
            ApertureType::Hole,
            Some(meter!(0.0, 0.0)),
            None,
        )?;
        let os = OpticSurface::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Sphere::new(
                meter!(1.0),
                Isometry::identity(),
            )?))),
            CoatingType::Fresnel,
            aperture,
            J_per_cm2!(2.0),
        )?;
        assert_eq!(os.lidt, J_per_cm2!(2.0));
        assert!(matches!(os.coating, CoatingType::Fresnel));
        assert!(matches!(os.aperture, _aperture));
        Ok(())
    }
    #[test]
    fn intersect_finds_where_a_ray_meets_the_surface() -> OpmResult<()> {
        // A sphere of 1 m radius with its vertex at the origin, curving away towards +z.
        let surface = OpticSurface::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Sphere::new(
                meter!(1.0),
                Isometry::new_along_z(meter!(1.0))?,
            )?))),
            CoatingType::IdealAR,
            Aperture::default(),
            J_per_cm2!(1.0),
        )?;
        let towards = Ray::new(
            meter!(0.0, 0.0, -1.0),
            nalgebra::Vector3::z(),
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        let (point, normal) = surface
            .intersect(&towards)?
            .ok_or_else(|| OpossumError::Other("the ray missed the surface".into()))?;
        assert!(point.coords.map(|c| c.value).norm() < 1e-15, "{point:?}");
        assert!(
            (normal - (-nalgebra::Vector3::z())).norm() < 1e-15,
            "{normal:?}"
        );
        // A ray running past the sphere sideways does not hit it.
        let past = Ray::new(
            meter!(2.0, 0.0, -1.0),
            nalgebra::Vector3::z(),
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        assert!(surface.intersect(&past)?.is_none());
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
