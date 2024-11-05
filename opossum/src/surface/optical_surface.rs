use log::warn;
use nalgebra::Point3;
use uom::si::f64::{Energy, Length};
use uuid::Uuid;

use super::{
    hit_map::{HitMap, RaysHitMap},
    GeoSurface, Plane,
};
use crate::{
    coatings::CoatingType, nodes::fluence_detector::Fluence, rays::Rays,
    utils::geom_transformation::Isometry, J_per_cm2,
};

/// This struct represents an optical surface, which consists of the geometric surface shape ([`GeoSurface`]) and further
/// properties such as the [`CoatingType`].
#[derive(Debug)]
pub struct OpticalSurface {
    geo_surface: Box<dyn GeoSurface>,
    coating: CoatingType,
    backward_rays_cache: Vec<Rays>,
    forward_rays_cache: Vec<Rays>,
    hit_map: HitMap,
    lidt: Fluence,
}
impl Default for OpticalSurface {
    fn default() -> Self {
        Self {
            geo_surface: Box::new(Plane::new(&Isometry::identity())),
            coating: CoatingType::IdealAR,
            backward_rays_cache: Vec::<Rays>::new(),
            forward_rays_cache: Vec::<Rays>::new(),
            hit_map: HitMap::default(),
            lidt: J_per_cm2!(1.0),
        }
    }
}
impl Clone for OpticalSurface {
    fn clone(&self) -> Self {
        Self {
            geo_surface: self.geo_surface.box_clone(),
            coating: self.coating.clone(),
            backward_rays_cache: self.backward_rays_cache.clone(),
            forward_rays_cache: self.forward_rays_cache.clone(),
            hit_map: self.hit_map.clone(),
            lidt: self.lidt,
        }
    }
}
impl OpticalSurface {
    /// Creates a new [`OpticalSurface`].
    #[must_use]
    pub fn new(geo_surface: Box<dyn GeoSurface>) -> Self {
        Self {
            geo_surface,
            coating: CoatingType::IdealAR,
            backward_rays_cache: Vec::<Rays>::new(),
            forward_rays_cache: Vec::<Rays>::new(),
            hit_map: HitMap::default(),
            lidt: J_per_cm2!(1.0),
        }
    }
    /// Returns a reference to the coating of this [`OpticalSurface`].
    #[must_use]
    pub const fn coating(&self) -> &CoatingType {
        &self.coating
    }
    /// Sets the coating of this [`OpticalSurface`].
    pub fn set_coating(&mut self, coating: CoatingType) {
        self.coating = coating;
    }
    /// Returns a reference to the geo surface of this [`OpticalSurface`].
    #[must_use]
    pub fn geo_surface(&self) -> &dyn GeoSurface {
        &(*self.geo_surface)
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
