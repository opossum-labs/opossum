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
    geometry::{Face, Geometry, Rim, SurfaceShape},
    light::{Ray, Rays},
    nodes::fluence_detector::Fluence,
    refractive_index::RefractiveIndexType,
    utils::geom_transformation::Isometry,
};
use core::fmt::Debug;
use nalgebra::{Point3, Vector3};
use std::sync::Arc;
use uom::si::f64::Length;

/// A face of a geometry placed at its node: the thing a surface belongs to, which decides where the
/// surface ends.
#[derive(Clone)]
struct PlacedFace {
    geometry: Arc<Geometry>,
    face: usize,
}

impl PlacedFace {
    /// Return whether a point on the unbounded surface of this face belongs to the component.
    fn includes(&self, point: &Point3<Length>) -> bool {
        self.geometry.kind().includes(self.face, point)
    }
}

/// This struct represents an optical surface, which consists of the geometric surface (a
/// [`Face`]) and further properties such as the [`CoatingType`].
#[derive(Clone)]
pub struct OpticSurface {
    surface: Face,
    window: Option<Rim>,
    face: Option<PlacedFace>,
    coating: CoatingType,
    lidt: Fluence,
    backward_rays_cache: Vec<Rays>,
    forward_rays_cache: Vec<Rays>,
    hit_map: HitMap,
}
impl Default for OpticSurface {
    /// Returns a default [`OpticSurface`].
    ///
    /// The default is an unbounded flat surface with an ideal antireflective coating (=no
    /// reflection) and a lidt of 1 J/cm².
    fn default() -> Self {
        Self {
            surface: Face::new(SurfaceShape::Plane, Isometry::identity()),
            window: None,
            face: None,
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
    pub fn new(surface: Face, coating: CoatingType, lidt: Fluence) -> OpmResult<Self> {
        if lidt.is_sign_negative() || lidt.is_nan() {
            return Err(OpossumError::Other(
                "LIDT must be positive and not NaN".into(),
            ));
        }
        Ok(Self {
            surface,
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
    /// Sets the geometric surface of this [`OpticSurface`].
    ///
    /// A surface set this way is no face of a placed description: it is unbounded from then on.
    pub fn set_surface(&mut self, surface: Face) {
        self.surface = surface;
        self.face = None;
    }
    /// Makes this [`OpticSurface`] a face of a geometry placed at its node.
    ///
    /// The geometric surface of the face and the window of a detector are taken from the geometry.
    /// Where the surface ends sideways is asked of the geometry, so the surface ends where the
    /// component does, whatever kind of component that is.
    ///
    /// # Arguments
    ///
    /// * `geometry` - the geometry, moved to its node and shared by all faces of the component.
    /// * `face` - the number of the face this surface is.
    ///
    /// # Errors
    ///
    /// This function returns an error if the geometry has no such face.
    pub fn set_face(&mut self, geometry: Arc<Geometry>, face: usize) -> OpmResult<()> {
        let kind = geometry.kind();
        let surface = kind.face(face).cloned().ok_or_else(|| {
            OpossumError::Other(format!(
                "there is no face {face}: the geometry has {} faces",
                kind.face_count()
            ))
        })?;
        self.window = kind.window().cloned();
        self.surface = surface;
        self.face = Some(PlacedFace { geometry, face });
        Ok(())
    }

    /// Sets the coating of this [`OpticSurface`].
    pub const fn set_coating(&mut self, coating: CoatingType) {
        self.coating = coating;
    }
    /// Returns the geometric surface of this [`OpticSurface`], placed in the world.
    #[must_use]
    pub const fn surface(&self) -> &Face {
        &self.surface
    }
    /// Returns the window of this [`OpticSurface`], or `None` if it records all light.
    ///
    /// The window of a detector ([`SurfaceExtent::Window`]) decides what is recorded (see
    /// [`OpticSurface::records_at`]), not what is hit.
    #[must_use]
    pub const fn window(&self) -> Option<&Rim> {
        self.window.as_ref()
    }
    /// Intersect a [`Ray`] with this [`OpticSurface`].
    ///
    /// A surface that is a face of a placed description (see [`OpticSurface::set_face`]) ends where
    /// that description says; the bodies of components decide the same way. Any other surface is
    /// unbounded.
    ///
    /// # Arguments
    ///
    /// * `ray` - the ray to be intersected.
    ///
    /// # Returns
    ///
    /// The intersection point in global coordinates and the normalized surface normal there,
    /// pointing against the ray; or `None` if the ray misses the surface or meets it where it is not
    /// part of the component.
    #[must_use]
    pub fn intersect(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        self.surface
            .intersect(ray)
            .filter(|(point, _)| self.face.as_ref().is_none_or(|face| face.includes(point)))
    }
    /// Return whether this [`OpticSurface`] records a hit at the given point.
    ///
    /// Only a detector's window ([`SurfaceExtent::Window`]) leaves points unrecorded: those outside
    /// it.
    ///
    /// # Arguments
    ///
    /// * `point` - a point on this surface, in global coordinates.
    #[must_use]
    pub fn records_at(&self, point: &Point3<Length>) -> bool {
        self.window.as_ref().is_none_or(|rim| rim.contains(point))
    }
    /// Cut a bundle of rays that passed this [`OpticSurface`] to what it records: the rays within
    /// a detector's window ([`SurfaceExtent::Window`]).
    ///
    /// # Arguments
    ///
    /// * `rays` - rays at this surface; those outside the window are invalidated.
    ///
    /// # Returns
    ///
    /// `true` if valid rays were cut.
    pub fn clip_to_window(&self, rays: &mut Rays) -> bool {
        self.window
            .as_ref()
            .is_some_and(|rim| rays.invalidate_outside(rim))
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
    pub fn propagate_rays(
        &mut self,
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
            .field("coating", &self.coating)
            .field("geometric surface", &self.surface)
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
        degree,
        error::{OpmResult, OpossumError},
        geometry::{Face, Geometry, ParametricSolid, Side, Solid, Sphere, SurfaceShape},
        joule,
        light::{Ray, Rays},
        meter, millimeter, nanometer,
        utils::geom_transformation::Isometry,
    };
    use std::sync::Arc;
    use uuid::Uuid;

    /// A circular outline of the given radius in millimeter.
    fn circle(radius: f64) -> OpmResult<Outline> {
        Outline::new(CircleShape::new(millimeter!(radius))?, Vec::new())
    }
    /// The face of the geometry with the given number, placed at the given node.
    fn face_of(geometry: &Geometry, node_frame: &Isometry, face: usize) -> OpmResult<OpticSurface> {
        let mut surface = OpticSurface::default();
        surface.set_face(Arc::new(geometry.moved(node_frame)), face)?;
        Ok(surface)
    }
    /// A plane with its vertex at the given pose of a node at the origin, its component reaching 5 mm
    /// around the node's axis.
    fn bounded_plane(vertex: Isometry) -> OpmResult<OpticSurface> {
        let geometry = Geometry::surface(
            Isometry::identity(),
            SurfaceShape::Plane,
            vertex,
            Some(circle(5.0)?),
        )?;
        face_of(&geometry, &Isometry::identity(), 0)
    }
    /// A ray along z, the given distance off the axis in y.
    fn ray_at(y: f64) -> OpmResult<Ray> {
        Ray::new_collimated(millimeter!(0.0, y, -10.0), nanometer!(1000.0), joule!(1.0))
    }
    #[test]
    fn intersect_misses_outside_the_rim() -> OpmResult<()> {
        let surface = bounded_plane(Isometry::identity())?;
        assert!(surface.intersect(&ray_at(4.9)?).is_some());
        assert!(surface.intersect(&ray_at(5.1)?).is_none());
        Ok(())
    }
    /// The surface pass tells for each ray of each bundle whether it hit. Ghost reflections cached
    /// at the surface left it from a hit, so when they are appended they count as hits.
    #[test]
    fn propagation_tells_which_rays_hit() -> OpmResult<()> {
        let mut surface = bounded_plane(Isometry::identity())?;
        surface.set_forward_rays_cache(vec![Rays::from(vec![ray_at(0.0)?, ray_at(7.0)?])]);
        let mut bundles = vec![Rays::from(vec![ray_at(4.9)?, ray_at(5.1)?])];
        let hits = surface.propagate_rays(
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
            bounded_plane(Isometry::identity())?.propagate_rays(
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
    #[test]
    fn the_rim_is_measured_in_the_node_frame() -> OpmResult<()> {
        // Tilted by 30°, the plane is hit at y = 4.9 mm by a ray along the node's axis. In the plane's
        // own frame that point lies 4.9 / cos 30° = 5.66 mm out, beyond the 5 mm rim.
        let surface = bounded_plane(Isometry::new(
            millimeter!(0.0, 0.0, 0.0),
            degree!(30.0, 0.0, 0.0),
        )?)?;
        assert!(surface.intersect(&ray_at(4.9)?).is_some());
        assert!(surface.intersect(&ray_at(5.1)?).is_none());
        Ok(())
    }
    /// A surface set by hand is not a face of a placed description, so nothing bounds it any more.
    #[test]
    fn a_surface_set_by_hand_is_unbounded() -> OpmResult<()> {
        let mut surface = bounded_plane(Isometry::identity())?;
        assert!(surface.intersect(&ray_at(7.0)?).is_none());
        surface.set_surface(Face::new(SurfaceShape::Plane, Isometry::identity()));
        assert!(surface.intersect(&ray_at(7.0)?).is_some());
        Ok(())
    }
    /// A face that is not there cannot be set, and leaves the surface as it was.
    #[test]
    fn a_face_that_does_not_exist_cannot_be_set() -> OpmResult<()> {
        let geometry = Geometry::plane(Some(circle(5.0)?));
        let mut surface = face_of(&geometry, &Isometry::identity(), 0)?;
        let error = surface
            .set_face(Arc::new(geometry.moved(&Isometry::identity())), 1)
            .unwrap_err();
        assert!(error.to_string().contains("no face 1"), "{error}");
        assert!(surface.intersect(&ray_at(4.9)?).is_some());
        assert!(surface.intersect(&ray_at(5.1)?).is_none());
        Ok(())
    }
    /// The window of a detector is no edge: a ray outside it hits the surface, it is just not recorded.
    #[test]
    fn a_detector_window_decides_what_is_recorded_not_what_is_hit() -> OpmResult<()> {
        let detector = Geometry::detector(SurfaceShape::Plane, Some(circle(5.0)?))?;
        let surface = face_of(&detector, &Isometry::identity(), 0)?;
        assert!(surface.window().is_some());
        assert!(surface.intersect(&ray_at(7.0)?).is_some());
        assert!(surface.records_at(&millimeter!(0.0, 4.9, 0.0)));
        assert!(!surface.records_at(&millimeter!(0.0, 5.1, 0.0)));
        Ok(())
    }
    /// A surface ends where its component does, whatever kind of component that is: the front face of
    /// this wedge-shaped solid ends where the side face cuts it, which no cross section states.
    #[test]
    fn a_face_of_a_parametric_solid_ends_at_the_other_faces() -> OpmResult<()> {
        let side = Isometry::new(millimeter!(5.0, 0.0, 5.0), degree!(0.0, 90.0, 0.0))?;
        let solid = Solid::Parametric(ParametricSolid::new(vec![
            (
                Face::new(SurfaceShape::Plane, Isometry::identity()),
                Side::Behind,
            ),
            (
                Face::new(
                    SurfaceShape::Plane,
                    Isometry::new_along_z(millimeter!(10.0))?,
                ),
                Side::InFront,
            ),
            (Face::new(SurfaceShape::Plane, side), Side::InFront),
        ])?);
        let front = face_of(&Geometry::Solid(solid), &Isometry::identity(), 0)?;
        let ray_at_x = |x: f64| {
            Ray::new_collimated(millimeter!(x, 0.0, -10.0), nanometer!(1000.0), joule!(1.0))
        };
        assert!(front.intersect(&ray_at_x(4.9)?).is_some());
        assert!(front.intersect(&ray_at_x(5.1)?).is_none());
        Ok(())
    }
    #[test]
    fn a_hit_behind_the_equator_is_a_miss() -> OpmResult<()> {
        // A sphere of 15 mm radius with its vertex at the node, bounded to 12.5 mm around the axis.
        let sphere = Geometry::surface(
            Isometry::identity(),
            SurfaceShape::Sphere(Sphere::new(millimeter!(15.0))?),
            Isometry::identity(),
            Some(circle(12.5)?),
        )?;
        let surface = face_of(&sphere, &Isometry::identity(), 0)?;
        // 135° from the vertex, the point lies behind the equator, 10.6 mm off the axis - within the
        // rim when seen along the axis. A ray tilted by 50° reaches the sphere there first.
        let (radius, polar, tilt) = (15.0, 135.0_f64.to_radians(), 50.0_f64.to_radians());
        let behind = (radius * polar.sin(), radius - radius * polar.cos());
        let direction = nalgebra::Vector3::new(-tilt.sin(), 0.0, tilt.cos());
        let steep = Ray::new(
            millimeter!(
                behind.0 - 30.0 * direction.x,
                0.0,
                behind.1 - 30.0 * direction.z
            ),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        assert!(surface.intersect(&steep).is_none());
        // A ray along the axis 5 mm out meets the sphere on the sheet through its vertex.
        assert!(surface.intersect(&ray_at(5.0)?).is_some());
        Ok(())
    }

    #[test]
    fn default() {
        let os = OpticSurface::default();
        assert!(matches!(os.coating, CoatingType::IdealAR));
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
        assert!(os.hit_map.is_empty());
        assert_eq!(os.lidt, J_per_cm2!(1.0));
    }
    #[test]
    fn new() -> OpmResult<()> {
        let gs = Face::new(SurfaceShape::Plane, Isometry::identity());
        assert!(OpticSurface::new(gs.clone(), CoatingType::IdealAR, J_per_cm2!(f64::NAN)).is_err());
        assert!(
            OpticSurface::new(
                gs.clone(),
                CoatingType::IdealAR,
                J_per_cm2!(f64::NEG_INFINITY)
            )
            .is_err()
        );
        assert!(OpticSurface::new(gs.clone(), CoatingType::IdealAR, J_per_cm2!(-0.1)).is_err());
        assert!(
            OpticSurface::new(gs.clone(), CoatingType::IdealAR, J_per_cm2!(f64::INFINITY)).is_ok()
        );

        let os = OpticSurface::new(
            Face::new(
                SurfaceShape::Sphere(Sphere::new(meter!(1.0))?),
                Isometry::identity(),
            ),
            CoatingType::Fresnel,
            J_per_cm2!(2.0),
        )?;
        assert_eq!(os.lidt, J_per_cm2!(2.0));
        assert!(matches!(os.coating, CoatingType::Fresnel));
        Ok(())
    }
    #[test]
    fn intersect_finds_where_a_ray_meets_the_surface() -> OpmResult<()> {
        // A sphere of 1 m radius with its vertex at the origin, curving away towards +z.
        let surface = OpticSurface::new(
            Face::new(
                SurfaceShape::Sphere(Sphere::new(meter!(1.0))?),
                Isometry::identity(),
            ),
            CoatingType::IdealAR,
            J_per_cm2!(1.0),
        )?;
        let towards = Ray::new(
            meter!(0.0, 0.0, -1.0),
            nalgebra::Vector3::z(),
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        let (point, normal) = surface
            .intersect(&towards)
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
        assert!(surface.intersect(&past).is_none());
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
