use log::warn;
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Energy, Length};
use uuid::Uuid;

use crate::{
    aperture::Aperture,
    coatings::CoatingType,
    error::OpmResult,
    nodes::fluence_detector::Fluence,
    rays::Rays,
    surface::{
        hit_map::{HitMap, RaysHitMap},
        GeoSurface, GeometricSurface,
    },
    utils::geom_transformation::Isometry,
    J_per_cm2,
};

use core::fmt::Debug;

#[derive(Serialize, Deserialize, Clone)]
pub struct OpticSurface {
    geo_surface: GeometricSurface,
    aperture: Aperture,
    coating: CoatingType,
    lidt: Fluence,

    backward_rays_cache: Vec<Rays>,
    forward_rays_cache: Vec<Rays>,
    hit_map: HitMap,
}

impl Default for OpticSurface {
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
    pub fn new(
        geo_surface: GeometricSurface,
        coating: CoatingType,
        aperture: Aperture,
        lidt: Fluence,
    ) -> Self {
        Self {
            geo_surface,
            aperture,
            coating,
            lidt,
            ..Default::default()
        }
    }
    pub fn get_rays_cache_mut(&mut self, get_back_ward_cache: bool) -> &mut Vec<Rays> {
        if get_back_ward_cache {
            &mut self.backward_rays_cache
        } else {
            &mut self.forward_rays_cache
        }
    }

    pub const fn get_rays_cache(&self, get_back_ward_cache: bool) -> &Vec<Rays> {
        if get_back_ward_cache {
            &self.backward_rays_cache
        } else {
            &self.forward_rays_cache
        }
    }
    pub fn set_geo_surface(&mut self, geo_surface: GeometricSurface) {
        self.geo_surface = geo_surface;
    }
    pub fn set_aperture(&mut self, aperture: Aperture) {
        self.aperture = aperture;
    }
    pub fn set_coating(&mut self, coating: CoatingType) {
        self.coating = coating;
    }
    pub const fn geo_surface(&self) -> &GeometricSurface {
        &self.geo_surface
    }
    pub const fn aperture(&self) -> &Aperture {
        &self.aperture
    }
    pub const fn coating(&self) -> &CoatingType {
        &self.coating
    }

    /// Sets the backwards rays cache of this [`OpticalSurface`].
    pub fn set_backwards_rays_cache(&mut self, backward_rays_cache: Vec<Rays>) {
        self.backward_rays_cache = backward_rays_cache;
    }
    /// Adds a rays bundle to the backwards rays cache of this [`OpticalSurface`].
    pub fn add_to_backward_rays_cache(&mut self, rays: Rays) {
        self.backward_rays_cache.push(rays);
    }
    /// Returns a reference to the backwards rays cache of this [`OpticalSurface`].
    #[must_use]
    pub const fn backwards_rays_cache(&self) -> &Vec<Rays> {
        &self.backward_rays_cache
    }
    /// Sets the forward rays cache of this [`OpticalSurface`].
    pub fn set_forward_rays_cache(&mut self, forward_rays_cache: Vec<Rays>) {
        self.forward_rays_cache = forward_rays_cache;
    }
    /// Adds a rays bundle to the forward rays cache of this [`OpticalSurface`].
    pub fn add_to_forward_rays_cache(&mut self, rays: Rays) {
        self.forward_rays_cache.push(rays);
    }
    /// Returns a reference to the forward rays cache of this [`OpticalSurface`].
    #[must_use]
    pub const fn forward_rays_cache(&self) -> &Vec<Rays> {
        &self.forward_rays_cache
    }

    /// Adds a rays bundle to the rays cache of this [`OpticalSurface`].
    pub fn add_to_rays_cache(&mut self, rays: Rays, add_to_forward_cache: bool) {
        if add_to_forward_cache {
            self.forward_rays_cache.push(rays);
        } else {
            self.backward_rays_cache.push(rays);
        }
    }
    /// Sets the isometry of this [`OpticalSurface`].
    pub fn set_isometry(&mut self, iso: &Isometry) {
        self.geo_surface.set_isometry(iso);
    }
    /// Returns a reference to the hit map of this [`OpticalSurface`].
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

    ///returns a reference to a [`RaysHitMap`] in this [`OpticalSurface`]
    #[must_use]
    pub fn get_rays_hit_map(&self, bounce: usize, uuid: &Uuid) -> Option<&RaysHitMap> {
        self.hit_map.get_rays_hit_map(bounce, uuid)
    }
    /// Add intersection point (with energy) to hit map.
    ///
    pub fn add_to_hit_map(
        &mut self,
        hit_point: (Point3<Length>, Energy),
        bounce: usize,
        rays_uuid: &Uuid,
    ) {
        self.hit_map.add_to_hitmap(hit_point, bounce, rays_uuid);
    }
    /// Reset hit map of this [`OpticalSurface`].
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

    ///returns a reference to the lidt value of this [`OpticalSurface`]
    #[must_use]
    pub fn lidt(&self) -> &Fluence {
        &self.lidt
    }
    ///set the lidt of this [`OpticalSurface`]
    pub fn set_lidt(&mut self, lidt: Fluence) {
        if lidt.is_sign_negative() || !lidt.is_normal() {
            warn!("LIDT values mut be > 0 and finite! Using default value of 1 J/cm²");
            return;
        }
        self.lidt = lidt;
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
