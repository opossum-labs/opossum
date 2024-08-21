//! Module for handling optical surface coatings

use crate::{error::OpmResult, ray::Ray};
use nalgebra::Vector3;

mod constant_r;
mod fresnel;
mod ideal_ar;

pub use constant_r::ConstantR;
pub use fresnel::Fresnel;
pub use ideal_ar::IdealAR;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CoatingType {
    /// Perfect anti-reflective coating. Reflectivity is always 0.0
    IdealAR,
    /// Ideal coating with a constant given reflectivity
    ConstantR { reflectivity: f64 },
    /// Fesnel reflection (e.g. uncaoted surface)
    Fresnel,
}
impl CoatingType {
    pub fn calc_reflectivity(
        &self,
        incoming_ray: &Ray,
        surface_normal: Vector3<f64>,
        n2: f64,
    ) -> OpmResult<f64> {
        match self {
            Self::IdealAR => {
                let c = IdealAR;
                Ok(c.calc_reflectivity(incoming_ray, surface_normal, n2))
            }
            Self::ConstantR { reflectivity } => {
                let c = ConstantR::new(*reflectivity)?;
                Ok(c.calc_reflectivity(incoming_ray, surface_normal, n2))
            }
            Self::Fresnel => {
                let c = Fresnel;
                Ok(c.calc_reflectivity(incoming_ray, surface_normal, n2))
            }
        }
    }
}
pub trait Coating {
    fn calc_reflectivity(&self, incoming_ray: &Ray, surface_normal: Vector3<f64>, n2: f64) -> f64;
    fn to_enum(&self) -> CoatingType;
}
