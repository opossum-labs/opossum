use nalgebra::Vector3;

use crate::ray::Ray;

use super::{Coating, CoatingType};

pub struct Fresnel;

impl Coating for Fresnel {
    fn calc_reflectivity(&self, incoming_ray: Ray, surface_normal: Vector3<f64>, n2: f64) -> f64 {
        // So far s-polarization only
        let alpha = incoming_ray.direction().angle(&surface_normal);
        let n1 = incoming_ray.refractive_index();
        let beta = f64::acos(f64::sqrt(n2*n2- n1*n1*f64::powi(f64::sin(alpha),2))/n2);
        f64::sin(alpha - beta) / f64::sin(alpha + beta)
    }

    fn to_enum(&self) -> super::CoatingType {
        CoatingType::Fresnel
    }
}
