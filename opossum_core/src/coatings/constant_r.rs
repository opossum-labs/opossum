#![warn(missing_docs)]
use super::{Coating, CoatingType};
use crate::generic_validators::AllInRange;
use crate::{error::OpmResult, light::Ray, validated, validated_type};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize)]
struct NonValidatedCoatingConstantR {
    pub reflectivity: f64,
}
/// Ein Type-Alias, um das Makro vor dem Utoipa-Parser zu verstecken.
pub type ValidatedReflectivity = validated_type!(f64, AllInRange<f64>);
impl Default for ValidatedReflectivity {
    fn default() -> Self {
        validated!(0.01, AllInRange::new(0.0, 1.0, true).unwrap()).unwrap()
    }
}

/// Ideal coating with constant reflectivity
///
/// The simple model represents an ideal coating with a given constant reflectivity independent from
/// the incoming wavelength, angle of incidence, or refractive index of the following medium.
#[derive(Default, Serialize, Debug, Clone, ToSchema, PartialEq)]
pub struct CoatingConstantR {
    reflectivity: ValidatedReflectivity,
}

impl CoatingConstantR {
    /// Create a new ideal coating with a given constant reflectivity.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given reflectivity is outside the interval [0.0,1.0] or not finite.
    pub fn new(reflectivity: f64) -> OpmResult<Self> {
        let mut new_reflectivity = ValidatedReflectivity::default();
        new_reflectivity.set(reflectivity)?;
        Ok(Self {
            reflectivity: new_reflectivity,
        })
    }
    /// Returns the reflectivity of this [`CoatingConstantR`].
    #[must_use]
    pub const fn reflectivity(&self) -> f64 {
        *self.reflectivity.get()
    }
}

impl<'de> serde::Deserialize<'de> for CoatingConstantR {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        //deserialize non validated struct
        let helper = NonValidatedCoatingConstantR::deserialize(deserializer)?;

        //get correct validators from default
        Self::new(helper.reflectivity).map_err(serde::de::Error::custom)
    }
}

impl Coating for CoatingConstantR {
    fn calc_reflectivity(
        &self,
        _incoming_ray: &Ray,
        _surface_normal: Vector3<f64>,
        _n2: f64,
    ) -> f64 {
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
        assert!(CoatingConstantR::new(-0.1).is_err());
        assert!(CoatingConstantR::new(f64::NAN).is_err());
        assert!(CoatingConstantR::new(f64::INFINITY).is_err());
        assert!(CoatingConstantR::new(f64::NEG_INFINITY).is_err());
        assert!(CoatingConstantR::new(1.0).is_ok());
        assert!(CoatingConstantR::new(1.0).is_ok());
        assert!(CoatingConstantR::new(1.1).is_err());
    }
    #[test]
    fn from() {
        let coating = CoatingConstantR::new(0.5).unwrap();
        if let CoatingType::ConstantR(config) = coating.into() {
            assert_eq!(*config.reflectivity.get(), 0.5);
        } else {
            panic!("Expected CoatingType::ConstantR variant");
        }
    }
    #[test]
    fn calc_refl() {
        let coating = CoatingConstantR::new(0.5).unwrap();
        let ray = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap();
        let surface_normal = vector![0.0, 0.0, -1.0];
        assert_eq!(coating.calc_reflectivity(&ray, surface_normal, 1.5), 0.5);
    }
}
