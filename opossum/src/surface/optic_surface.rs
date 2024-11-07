//! Module handling optical surfaces
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Energy, Length};
use uuid::Uuid;

use crate::{
    aperture::Aperture,
    coatings::CoatingType,
    error::{OpmResult, OpossumError},
    nodes::fluence_detector::Fluence,
    rays::Rays,
    surface::{
        geo_surface::{GeoSurface, GeometricSurface},
        hit_map::{HitMap, RaysHitMap},
    },
    utils::geom_transformation::Isometry,
    J_per_cm2,
};

use core::fmt::Debug;

/// This struct represents an optical surface, which consists of the geometric surface shape ([`GeoSurface`]) and further
/// properties such as the [`CoatingType`].
#[derive(Serialize, Deserialize, Clone)]
pub struct OpticSurface {
    #[serde(skip)]
    geo_surface: GeometricSurface,
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
    /// The default is a flat surface with an ideal antireflective caoting (=no reflection), no limiting aperture
    /// and a lidt of 1 J/cm².
    fn default() -> Self {
        Self {
            geo_surface: GeometricSurface::default(),
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
        geo_surface: GeometricSurface,
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
    pub fn set_geo_surface(&mut self, geo_surface: GeometricSurface) {
        self.geo_surface = geo_surface;
    }
    /// Sets the aperture of this [`OpticSurface`].
    pub fn set_aperture(&mut self, aperture: Aperture) {
        self.aperture = aperture;
    }
    /// Sets the coating of this [`OpticSurface`].
    pub fn set_coating(&mut self, coating: CoatingType) {
        self.coating = coating;
    }
    /// Returns a reference to the geo surface of this [`OpticSurface`].
    #[must_use]
    pub const fn geo_surface(&self) -> &GeometricSurface {
        &self.geo_surface
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
    pub fn set_isometry(&mut self, iso: &Isometry) {
        self.geo_surface.set_isometry(iso);
    }
    /// Returns a reference to the hit map of this [`OpticSurface`].
    ///
    /// This function returns a vector of intersection points (with energies) of [`Rays`] that hit the surface.
    #[must_use]
    pub const fn hit_map(&self) -> &HitMap {
        &self.hit_map
    }
    ///stores a critical fluence in a hitmap
    pub fn add_critical_fluence(
        &mut self,
        uuid: &Uuid,
        rays_hist_pos: usize,
        fluence: Fluence,
        bounce: usize,
    ) {
        self.hit_map
            .add_critical_fluence(uuid, rays_hist_pos, fluence, bounce);
    }

    ///returns a reference to a [`RaysHitMap`] in this [`OpticSurface`]
    #[must_use]
    pub fn get_rays_hit_map(&self, bounce: usize, uuid: &Uuid) -> Option<&RaysHitMap> {
        self.hit_map.get_rays_hit_map(bounce, uuid)
    }
    /// Add intersection point (with energy) to hit map.
    ///
    /// # Errors
    ///
    /// This function retruns an error if the given hit point is invalid.
    pub fn add_to_hit_map(
        &mut self,
        hit_point: (Point3<Length>, Energy),
        bounce: usize,
        rays_uuid: &Uuid,
    ) -> OpmResult<()> {
        self.hit_map.add_to_hitmap(hit_point, bounce, rays_uuid)
    }
    /// Reset hit map of this [`OpticSurface`].
    pub fn reset_hit_map(&mut self) {
        self.hit_map.reset();
    }

    /// Evaluate the fluence of a given ray bundle on this surface. If the fluence surpasses its lidt, store the critical fluence parameters in the hitmap
    /// # Errors
    /// This function errors  on error propagation of `calc_fluence`
    pub fn evaluate_fluence_of_ray_bundle(&mut self, rays: &Rays) -> OpmResult<()> {
        if let Some(rays_hit_map) = self.get_rays_hit_map(rays.bounce_lvl(), rays.uuid()) {
            if let Some((_, _, _, _, peak_fluence)) =
                rays_hit_map.calc_fluence_with_voronoi_method(self.lidt)?
            {
                self.add_critical_fluence(
                    rays.uuid(),
                    rays.ray_history_len(),
                    peak_fluence,
                    rays.bounce_lvl(),
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
    /// Sets the laser induced damage threshold (LIDT) [`OpticSurface`]
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
            .field("aperture", &self.aperture)
            .field("coating", &self.coating)
            .field("geometric surface", &self.geo_surface)
            .field("backward rays cache", &self.backward_rays_cache)
            .field("forward rays cache", &self.forward_rays_cache)
            .field("hitmap", &self.hit_map)
            .field("lidt", &self.lidt)
            .finish()
    }
}

#[cfg(test)]
mod test {
    use super::OpticSurface;
    use crate::{
        aperture::{Aperture, CircleConfig},
        coatings::CoatingType,
        joule, meter, nanometer,
        ray::Ray,
        rays::Rays,
        surface::{geo_surface::GeometricSurface, Sphere},
        utils::geom_transformation::Isometry,
        J_per_cm2,
    };
    use core::f64;
    use uuid::Uuid;

    #[test]
    fn default() {
        let os = OpticSurface::default();
        assert!(matches!(os.aperture, Aperture::None));
        assert!(matches!(os.coating, CoatingType::IdealAR));
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
        assert!(os.hit_map.is_empty());
        assert_eq!(os.lidt, J_per_cm2!(1.0));
    }
    #[test]
    fn new() {
        let gs = GeometricSurface::default();
        assert!(OpticSurface::new(
            gs.clone(),
            CoatingType::IdealAR,
            Aperture::None,
            J_per_cm2!(f64::NAN)
        )
        .is_err());
        assert!(OpticSurface::new(
            gs.clone(),
            CoatingType::IdealAR,
            Aperture::None,
            J_per_cm2!(f64::NEG_INFINITY)
        )
        .is_err());
        assert!(OpticSurface::new(
            gs.clone(),
            CoatingType::IdealAR,
            Aperture::None,
            J_per_cm2!(-0.1)
        )
        .is_err());
        assert!(OpticSurface::new(
            gs.clone(),
            CoatingType::IdealAR,
            Aperture::None,
            J_per_cm2!(f64::INFINITY)
        )
        .is_ok());

        let aperture =
            Aperture::BinaryCircle(CircleConfig::new(meter!(1.0), meter!(0.0, 0.0)).unwrap());
        let os = OpticSurface::new(
            GeometricSurface::Spherical {
                s: Sphere::new(meter!(1.0), &Isometry::identity()).unwrap(),
            },
            CoatingType::Fresnel,
            aperture,
            J_per_cm2!(2.0),
        )
        .unwrap();
        assert_eq!(os.lidt, J_per_cm2!(2.0));
        assert!(matches!(os.coating, CoatingType::Fresnel));
        assert!(matches!(os.aperture, _aperture));
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
    fn add_to_rays_cache() {
        let mut os = OpticSurface::default();
        let ray =
            Ray::new_collimated(meter!(0.0, 0.0, 0.0), nanometer!(1000.0), joule!(1.0)).unwrap();
        let mut rays = Rays::default();
        rays.add_ray(ray);
        os.add_to_rays_cache(rays.clone(), true);
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 1);
        os.add_to_rays_cache(rays.clone(), false);
        assert_eq!(os.backward_rays_cache.len(), 1);
        assert_eq!(os.forward_rays_cache.len(), 1);
    }
    #[test]
    fn set_backwards_rays_cache() {
        let mut os = OpticSurface::default();
        let ray =
            Ray::new_collimated(meter!(0.0, 0.0, 0.0), nanometer!(1000.0), joule!(1.0)).unwrap();
        let mut rays = Rays::default();
        rays.add_ray(ray);
        os.set_backwards_rays_cache(vec![rays]);
        assert_eq!(os.backward_rays_cache.len(), 1);
        assert_eq!(os.forward_rays_cache.len(), 0);
        os.set_backwards_rays_cache(vec![]);
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
    }
    #[test]
    fn set_forwards_rays_cache() {
        let mut os = OpticSurface::default();
        let ray =
            Ray::new_collimated(meter!(0.0, 0.0, 0.0), nanometer!(1000.0), joule!(1.0)).unwrap();
        let mut rays = Rays::default();
        rays.add_ray(ray);
        os.set_forward_rays_cache(vec![rays]);
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 1);
        os.set_forward_rays_cache(vec![]);
        assert_eq!(os.backward_rays_cache.len(), 0);
        assert_eq!(os.forward_rays_cache.len(), 0);
    }
    #[test]
    fn add_critical_fluence() {
        let mut os = OpticSurface::default();
        let uuid = Uuid::new_v4();
        os.add_critical_fluence(&uuid, 1, J_per_cm2!(1.0), 2);
        let hit_map = os.hit_map();
        assert!(hit_map.critical_fluences().get(&Uuid::nil()).is_none());
        let critical_fluence = hit_map.critical_fluences().get(&uuid).unwrap();
        assert_eq!(critical_fluence.0, J_per_cm2!(1.0));
        assert_eq!(critical_fluence.1, 1);
        assert_eq!(critical_fluence.2, 2);
    }
}
