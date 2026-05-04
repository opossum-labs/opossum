#![warn(missing_docs)]
//! Module for handling optical surface coatings

use crate::{error::OpmResult, light::Ray, utils::default_from_name::DefaultFromName};
use nalgebra::Vector3;
use std::fmt::Display;
use uom::si::f64::Ratio;
mod constant_r;
mod fresnel;
mod ideal_ar;

pub use constant_r::CoatingConstantR;
pub use fresnel::Fresnel;
pub use ideal_ar::IdealAR;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};
use utoipa::ToSchema;

#[derive(Default, Serialize, Deserialize, Debug, Clone, ToSchema, PartialEq, EnumIter, Copy)]
/// Enum for different types of optical coatings
pub enum CoatingType {
    /// Perfect anti-reflective coating. Reflectivity is always 0.0
    #[default]
    IdealAR,
    /// Ideal coating with a constant given reflectivity
    ConstantR(CoatingConstantR),
    /// Fesnel reflection (e.g. uncoated surface)
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
    ) -> OpmResult<Ratio> {
        match self {
            Self::IdealAR => {
                let c = IdealAR;
                Ok(c.calc_reflectivity(incoming_ray, surface_normal, n2))
            }
            Self::ConstantR(refl_config) => {
                Ok(refl_config.calc_reflectivity(incoming_ray, surface_normal, n2))
            }
            Self::Fresnel => {
                let c = Fresnel;
                Ok(c.calc_reflectivity(incoming_ray, surface_normal, n2))
            }
        }
    }
}
impl Display for CoatingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConstantR { .. } => write!(f, "Constant Reflectivity"),
            Self::IdealAR => write!(f, "Ideal AR"),
            Self::Fresnel => write!(f, "uncoated (Fresnel)"),
        }
    }
}
impl DefaultFromName for CoatingType {
    fn default_from_name(name: &str) -> Option<Self> {
        for ct in Self::iter() {
            if name == format!("{ct}") {
                match ct {
                    Self::ConstantR { .. } => {
                        return Some(Self::ConstantR(CoatingConstantR::default()));
                    }
                    Self::IdealAR => return Some(Self::IdealAR),
                    Self::Fresnel => return Some(Self::Fresnel),
                }
            }
        }
        None
    }
}
/// Trait for optical coatings
///
/// Each coating model must implement this trait to be used in the ray tracing simulation.
pub trait Coating {
    /// Calculate the reflectivity based on the concrete model for an incoming [`Ray`] on a surface with
    /// a given `surface_normal` at the intersection point and the refractive index of the following medium.
    fn calc_reflectivity(&self, incoming_ray: &Ray, surface_normal: Vector3<f64>, n2: f64)
    -> Ratio;
}
