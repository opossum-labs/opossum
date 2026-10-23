#![warn(missing_docs)]
//! Module for handling optical surface coatings

use crate::{
    error::{OpmResult, OpossumError},
    light::Ray,
    utils::default_from_name::DefaultFromName,
};
use nalgebra::Vector3;
use std::fmt::Display;
use uom::si::f64::Ratio;
mod constant_r;
mod fresnel;
mod ideal_ar;
mod separator;

pub use constant_r::CoatingConstantR;
pub use fresnel::Fresnel;
pub use ideal_ar::IdealAR;
pub use separator::{SplittingConfig, SplittingConfigBuilder};
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

#[derive(Default, Serialize, Deserialize, Debug, Clone, PartialEq, EnumIter)]
/// Enum for different types of optical coatings
pub enum CoatingType {
    /// Perfect anti-reflective coating. Reflectivity is always 0.0
    #[default]
    IdealAR,
    /// Ideal coating with a constant given reflectivity
    ConstantR(CoatingConstantR),
    /// Fesnel reflection (e.g. uncoated surface)
    Fresnel,
    /// Splits by a [`SplittingConfig`]: transmits a fixed ratio or a spectrum and reflects the rest,
    /// whatever the angle of incidence
    Separator(SplittingConfigBuilder),
}
impl CoatingType {
    /// Return whether this coating is physically possible: a separator must transmit within
    /// `[0.0, 1.0]`. A constant reflectivity is checked when it is created.
    ///
    /// # Errors
    ///
    /// This function returns an error if the transmission spectrum of a separator cannot be read
    /// from its file.
    pub fn is_valid(&self) -> OpmResult<bool> {
        match self {
            Self::IdealAR | Self::ConstantR(_) | Self::Fresnel => Ok(true),
            Self::Separator(config) => config.is_valid(),
        }
    }
}
impl TryFrom<&CoatingType> for Box<dyn Coating> {
    type Error = OpossumError;
    /// Build the coating model of a [`CoatingType`], once for all rays of an interaction with a
    /// surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the coating model cannot be built from its definition.
    fn try_from(coating: &CoatingType) -> OpmResult<Self> {
        Ok(match coating {
            CoatingType::IdealAR => Box::new(IdealAR),
            CoatingType::ConstantR(constant_r) => Box::new(*constant_r),
            CoatingType::Fresnel => Box::new(Fresnel),
            CoatingType::Separator(config) => Box::new(config.build()?),
        })
    }
}
impl Display for CoatingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConstantR { .. } => write!(f, "Constant Reflectivity"),
            Self::IdealAR => write!(f, "Ideal AR"),
            Self::Fresnel => write!(f, "uncoated (Fresnel)"),
            Self::Separator(_) => write!(f, "Separator"),
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
                    Self::Separator(_) => {
                        return Some(Self::Separator(SplittingConfigBuilder::default()));
                    }
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
    ///
    /// # Errors
    ///
    /// This function returns an error if the model defines no reflectivity for the ray, for instance at
    /// its wavelength.
    fn calc_reflectivity(
        &self,
        incoming_ray: &Ray,
        surface_normal: Vector3<f64>,
        n2: f64,
    ) -> OpmResult<Ratio>;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, nanometer, percent};
    use approx::assert_abs_diff_eq;

    #[test]
    fn built_model_reflects_like_its_coating_type() -> OpmResult<()> {
        let mut ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0))?;
        ray.set_refractive_index(1.0)?;
        let reflectivity = |coating: CoatingType| -> OpmResult<f64> {
            Ok(Box::<dyn Coating>::try_from(&coating)?
                .calc_reflectivity(&ray, -Vector3::z(), 1.5)?
                .value)
        };
        assert_abs_diff_eq!(reflectivity(CoatingType::IdealAR)?, 0.0);
        assert_abs_diff_eq!(
            reflectivity(CoatingConstantR::new(percent!(20.0))?.into())?,
            0.2
        );
        assert_abs_diff_eq!(reflectivity(CoatingType::Fresnel)?, 0.04);
        assert_abs_diff_eq!(
            reflectivity(SplittingConfigBuilder::FixedRatio(0.6).into())?,
            0.4
        );
        Ok(())
    }
    #[test]
    fn every_coating_type_has_a_default_by_its_name() {
        for coating in CoatingType::iter() {
            assert!(
                CoatingType::default_from_name(&coating.to_string()).is_some(),
                "{coating}"
            );
        }
    }
    #[test]
    fn a_separator_must_transmit_within_zero_and_one() -> OpmResult<()> {
        for transmission in [0.0, 0.5, 1.0] {
            assert!(CoatingType::Separator(transmission.into()).is_valid()?);
        }
        for transmission in [-0.01, 1.01] {
            assert!(!CoatingType::Separator(transmission.into()).is_valid()?);
        }
        Ok(())
    }
}
