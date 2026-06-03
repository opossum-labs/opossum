#![warn(missing_docs)]
use super::{Coating, CoatingType};
use crate::percent;
use crate::{
    error::OpmResult,
    generic_validators::{AllInRange, ValidateTrait},
    light::Ray,
    validated, validated_type,
};
use nalgebra::Vector3;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Ratio;
use utoipa::ToSchema;

#[derive(Deserialize)]
struct NonValidatedCoatingConstantR {
    pub reflectivity: Ratio,
}

impl TryFrom<NonValidatedCoatingConstantR> for CoatingConstantR {
    type Error = String;
    fn try_from(helper: NonValidatedCoatingConstantR) -> Result<Self, Self::Error> {
        Self::new(helper.reflectivity).map_err(|e| e.to_string())
    }
}

pub type ValidatedReflectivity = validated_type!(Ratio, AllInRange<Ratio>);
impl Default for ValidatedReflectivity {
    fn default() -> Self {
        validated!(
            percent!(1.0),
            AllInRange::new(percent!(0.0), percent!(100.0), true).unwrap()
        )
        .unwrap()
    }
}

/// Ideal coating with constant reflectivity
///
/// The simple model represents an ideal coating with a given constant reflectivity independent from
/// the incoming wavelength, angle of incidence, or refractive index of the following medium.
#[derive(
    Default, Deserialize, Serialize, Debug, Clone, ToSchema, PartialEq, EnsureValidated, Copy,
)]
#[serde(try_from = "NonValidatedCoatingConstantR")]
pub struct CoatingConstantR {
    /// The reflectivity of the coating in the range [0.0, 1.0].
    #[schema(value_type = f64, example = 0.5)]
    reflectivity: ValidatedReflectivity,
}

impl CoatingConstantR {
    /// Create a new ideal coating with a given constant reflectivity.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given reflectivity is outside the interval [0.0,1.0] or not finite.
    pub fn new(reflectivity: Ratio) -> OpmResult<Self> {
        let mut new_reflectivity = ValidatedReflectivity::default();
        new_reflectivity.set(reflectivity)?;
        Ok(Self {
            reflectivity: new_reflectivity,
        })
    }
    /// Returns the reflectivity of this [`CoatingConstantR`].
    #[must_use]
    pub const fn reflectivity(&self) -> Ratio {
        *self.reflectivity.get()
    }
}

impl Coating for CoatingConstantR {
    fn calc_reflectivity(
        &self,
        _incoming_ray: &Ray,
        _surface_normal: Vector3<f64>,
        _n2: f64,
    ) -> Ratio {
        *self.reflectivity.get()
    }
}
impl From<CoatingConstantR> for CoatingType {
    fn from(coating: CoatingConstantR) -> Self {
        Self::ConstantR(coating)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, light::Ray, nanometer};
    use core::f64;
    use nalgebra::vector;

    #[test]
    fn new() {
        assert!(CoatingConstantR::new(percent!(-0.1)).is_err());
        assert!(CoatingConstantR::new(percent!(f64::NAN)).is_err());
        assert!(CoatingConstantR::new(percent!(f64::INFINITY)).is_err());
        assert!(CoatingConstantR::new(percent!(f64::NEG_INFINITY)).is_err());
        assert!(CoatingConstantR::new(percent!(0.0)).is_ok());
        assert!(CoatingConstantR::new(percent!(100.0)).is_ok());
        assert!(CoatingConstantR::new(percent!(100.1)).is_err());
    }
    #[test]
    fn from() -> OpmResult<()> {
        let coating = CoatingConstantR::new(percent!(50.0))?;
        if let CoatingType::ConstantR(config) = coating.into() {
            assert_eq!(*config.reflectivity.get(), percent!(50.0));
        } else {
            panic!("Expected CoatingType::ConstantR variant");
        }
        Ok(())
    }
    #[test]
    fn calc_refl() -> OpmResult<()> {
        let coating = CoatingConstantR::new(percent!(50.0))?;
        let ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0))?;
        let surface_normal = vector![0.0, 0.0, -1.0];
        assert_eq!(
            coating.calc_reflectivity(&ray, surface_normal, 1.5),
            percent!(50.0)
        );
        Ok(())
    }
}
