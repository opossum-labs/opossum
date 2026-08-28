use crate::error::OpmResult;
use crate::generic_validators::{AllFinite, StaticBounds, StaticInRange};
use crate::{validated, validated_type};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Eq)]
pub struct AbsBounds;

impl StaticBounds<f64> for AbsBounds {
    fn min() -> f64 {
        0.0
    }
    fn max() -> f64 {
        1.0
    }
    fn inclusive() -> bool {
        true
    }
}

pub type ValidatedAbsConst = validated_type!(f64, AllFinite && StaticInRange::<f64, AbsBounds>);

impl Default for ValidatedAbsConst {
    fn default() -> Self {
        validated!(1.0, AllFinite && StaticInRange::<f64, AbsBounds>::default()).unwrap()
    }
}

#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq, Copy, EnsureValidated)]
pub struct AbsConst {
    absorption_constant: ValidatedAbsConst,
}
impl AbsConst {
    /// Create a constant absorption factor [`AbsConst`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the given factor is outside the interval `[0.0..1.0]`.
    pub fn new(factor: f64) -> OpmResult<Self> {
        let mut abs_const = Self::default();
        abs_const.absorption_constant.set(factor)?;
        Ok(abs_const)
    }
    #[must_use]
    pub const fn absorption_constant(&self) -> f64 {
        *self.absorption_constant.get()
    }
}
