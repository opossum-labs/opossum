//! Trivial constant refractive index model
//!
//! This model simply returns a wavelength independant constant value.
use opm_macros_lib::EnsureValidated;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use super::{RefractiveIndex, RefractiveIndexType};
use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllInRange, ValidateTrait},
    validated, validated_type,
};
#[derive(Deserialize)]
struct NonValidatedRefrIndexConst {
    pub refractive_index: f64,
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
/// Create a refractive index model representing vacuum.
///
/// This returns a constant (wavelength independant) refractive index of 1.0.
pub fn refr_index_vaccuum() -> RefractiveIndexType {
    RefractiveIndexType::Const(RefrIndexConst::new(1.0).unwrap())
}

type ValidatedRefIndCOnst = validated_type!(f64, AllFinite && AllInRange::<f64>);

/// Constant refractive index model
#[derive(Clone, Serialize, ToSchema, Debug, PartialEq, Copy, EnsureValidated)]
pub struct RefrIndexConst {
    refractive_index: ValidatedRefIndCOnst,
}

impl<'de> serde::Deserialize<'de> for RefrIndexConst {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        //deserialize non validated struct
        let helper = NonValidatedRefrIndexConst::deserialize(deserializer)?;

        //get correct validators from default
        Self::new(helper.refractive_index).map_err(serde::de::Error::custom)
    }
}

fn default_ref_ind_const() -> ValidatedRefIndCOnst {
    validated!(
        1.5,
        AllFinite && (AllInRange::new(1.0, f64::INFINITY, true).unwrap())
    )
    .unwrap()
}
impl RefrIndexConst {
    /// Create a new constant refrective index model.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given refractive index is < 1.0 or not finite.
    pub fn new(refractive_index: f64) -> OpmResult<Self> {
        let mut ref_ind = Self::default();
        ref_ind.set_refractive_index(refractive_index)?;

        Ok(ref_ind)
    }

    /// Get the refractive index value.
    #[must_use]
    pub const fn refractive_index(&self) -> f64 {
        *self.refractive_index.get()
    }

    /// Set the refractive index value.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_refractive_index(&mut self, ref_ind: f64) -> OpmResult<()> {
        self.refractive_index.set(ref_ind)?;
        Ok(())
    }
}

impl Default for RefrIndexConst {
    fn default() -> Self {
        Self {
            refractive_index: default_ref_ind_const(),
        }
    }
}

impl RefractiveIndex for RefrIndexConst {
    fn get_refractive_index(&self, _wavelength: uom::si::f64::Length) -> OpmResult<f64> {
        Ok(self.refractive_index())
    }
}
impl From<RefrIndexConst> for RefractiveIndexType {
    fn from(i: RefrIndexConst) -> Self {
        Self::Const(i)
    }
}
#[cfg(test)]
mod test {
    use num::Zero;
    use uom::si::f64::Length;

    use super::*;
    #[test]
    fn new() {
        assert!(RefrIndexConst::new(0.99).is_err());
        assert!(RefrIndexConst::new(f64::NAN).is_err());
        assert!(RefrIndexConst::new(f64::INFINITY).is_err());
    }
    #[test]
    fn get_refractive_index() {
        let i = RefrIndexConst::new(1.5).unwrap();
        assert_eq!(i.get_refractive_index(Length::zero()).unwrap(), 1.5);
    }
    #[test]
    fn get_enum() {
        let i = RefrIndexConst::new(1.5).unwrap();
        assert!(matches!(
            RefractiveIndexType::from(&i),
            RefractiveIndexType::Const(_)
        ));
    }

    #[test]
    fn validator_deserialize() {
        let i = RefrIndexConst::new(1.5).unwrap();
        let serialized =
            ron::ser::to_string_pretty(&i, ron::ser::PrettyConfig::new().new_line("\n")).unwrap();
        let mut deserialized: RefrIndexConst = ron::from_str(&serialized).unwrap();

        // all in range must still be valid by the default setter!
        assert_eq!(deserialized.refractive_index(), 1.5);
        assert!(deserialized.set_refractive_index(2.5).is_ok());
        assert_eq!(deserialized.refractive_index(), 2.5);
        assert!(deserialized.set_refractive_index(0.9).is_err());
        assert_eq!(deserialized.refractive_index(), 2.5);
    }
}
