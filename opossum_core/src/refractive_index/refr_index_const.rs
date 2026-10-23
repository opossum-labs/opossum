//! Trivial constant refractive index model
//!
//! This model simply returns a wavelength independant constant value.
use opm_macros_lib::EnsureValidated;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use super::{RefractiveIndex, RefractiveIndexType};
use crate::generic_validators::StaticBounds;
use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, StaticInRange},
    validated, validated_type,
};

#[derive(Copy, Clone, PartialEq, Debug, Eq)]
struct RefIndBounds;

impl StaticBounds<f64> for RefIndBounds {
    fn min() -> f64 {
        1.0
    }
    fn max() -> f64 {
        f64::INFINITY
    }
    fn inclusive() -> bool {
        true
    }
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
/// Create a refractive index model representing vacuum.
///
/// This returns a constant (wavelength independant) refractive index of 1.0.
pub fn refr_index_vaccuum() -> RefractiveIndexType {
    RefractiveIndexType::Const(RefrIndexConst::new(1.0).unwrap())
}

type ValidatedRefIndConst = validated_type!(f64, AllFinite && StaticInRange::<f64, RefIndBounds>);
impl Default for ValidatedRefIndConst {
    fn default() -> Self {
        validated!(
            1.5,
            AllFinite && StaticInRange::<f64, RefIndBounds>::default()
        )
        .unwrap()
    }
}

/// Constant refractive index model
#[derive(
    Default, Clone, Serialize, Deserialize, ToSchema, Debug, PartialEq, Copy, EnsureValidated,
)]
pub struct RefrIndexConst {
    refractive_index: ValidatedRefIndConst,
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
    use num_traits::Zero;
    use uom::si::f64::Length;

    use crate::error::OpossumError;

    use super::*;
    #[test]
    fn new() {
        assert!(RefrIndexConst::new(0.99).is_err());
        assert!(RefrIndexConst::new(f64::NAN).is_err());
        assert!(RefrIndexConst::new(f64::INFINITY).is_err());
    }
    #[test]
    fn get_refractive_index() -> OpmResult<()> {
        let i = RefrIndexConst::new(1.5)?;
        assert_eq!(i.get_refractive_index(Length::zero())?, 1.5);
        Ok(())
    }
    #[test]
    fn get_enum() -> OpmResult<()> {
        let i = RefrIndexConst::new(1.5)?;
        assert!(matches!(
            RefractiveIndexType::from(&i),
            RefractiveIndexType::Const(_)
        ));
        Ok(())
    }

    #[test]
    fn validator_deserialize() -> OpmResult<()> {
        let i = RefrIndexConst::new(1.5)?;
        let serialized =
            ron::ser::to_string_pretty(&i, ron::ser::PrettyConfig::new().new_line("\n"))
                .map_err(|e| OpossumError::Other(format!("serialization error: {e}")))?;
        let mut deserialized: RefrIndexConst = ron::from_str(&serialized)
            .map_err(|e| OpossumError::Other(format!("deserialization error: {e}")))?;

        // all in range must still be valid by the default setter!
        assert_eq!(deserialized.refractive_index(), 1.5);
        assert!(deserialized.set_refractive_index(2.5).is_ok());
        assert_eq!(deserialized.refractive_index(), 2.5);
        assert!(deserialized.set_refractive_index(0.9).is_err());
        assert_eq!(deserialized.refractive_index(), 2.5);
        Ok(())
    }
}
