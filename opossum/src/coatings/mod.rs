//! Module for handling optical surface coatings

use crate::ray::Ray;
use nalgebra::Vector3;

mod ideal_ar;
mod constant_r;

pub use ideal_ar::IdealAR;
pub use constant_r::ConstantR;

pub enum CoatingType {
    /// Perfect anti-reflective coating. Reflectivity is always 0.0 
    IdealAR,
    /// Ideal coating with a constant given reflectivity
    ConstantR { reflectivity: f64 },
    /// Fesnel reflection (e.g. uncaoted surface)
    Fresnel,
}

pub trait Coating {
    fn calc_reflectivity(&self, incoming_ray: Ray, surface_normal: Vector3<f64>) -> f64;
    fn to_enum(&self) -> CoatingType;
}
