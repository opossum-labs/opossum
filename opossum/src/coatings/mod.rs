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
    /// Calculate the reflectivity of a coating hit by a given [`Ray`] on a [`GeoSurface`](crate::surface::geo_surface::GeoSurface)
    /// characterized by the given surface normal at the intersection point.
    ///
    /// # Errors
    ///
    /// This function will return an error if the underlying concrete implementation returns an error.
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
    /// Calculate the reflectivity based on the concrete model for an incoming [`Ray`] on a surface with
    /// a given `surface_normal` at the intersection point and the refractive index of the following medium.
    fn calc_reflectivity(&self, incoming_ray: &Ray, surface_normal: Vector3<f64>, n2: f64) -> f64;
    /// Return the corresponding [`CoatingType`] for a given [`Coating`].
    ///
    /// This function is mainly used for serialization / deserialization.
    fn to_enum(&self) -> CoatingType;
}
