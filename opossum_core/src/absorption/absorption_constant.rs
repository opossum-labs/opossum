use crate::generic_validators::ValidateTrait;
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

type ValidatedAbsConst = validated_type!(f64, AllFinite && StaticInRange::<f64, AbsBounds>);

impl Default for ValidatedAbsConst {
    fn default() -> Self {
        validated!(1.0, AllFinite && StaticInRange::<f64, AbsBounds>::default()).unwrap()
    }
}

#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq, Copy, EnsureValidated)]
pub struct AbsConst {
    absoprtion_constant: ValidatedAbsConst,
}
impl AbsConst {}
