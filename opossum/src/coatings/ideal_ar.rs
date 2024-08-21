
use super::{Coating, CoatingType};

pub struct IdealAR;

impl Coating for IdealAR {
    fn calc_reflectivity(
        &self,
        _incoming_ray: crate::ray::Ray,
        _surface_normal: nalgebra::Vector3<f64>,
        _n2: f64,
    ) -> f64 {
        0.0
    }

    fn to_enum(&self) -> super::CoatingType {
        CoatingType::IdealAR
    }
}
