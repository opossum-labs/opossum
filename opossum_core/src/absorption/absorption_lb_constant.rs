use crate::{error::OpmResult, generic_validators::*, num_per_m, validated, validated_type};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::LinearNumberDensity;

type ValidatedLBConst = validated_type!(LinearNumberDensity, AllFinite && AllPositive);
impl Default for ValidatedLBConst {
    fn default() -> Self {
        validated!(num_per_m!(0.0), AllFinite && AllPositive).unwrap()
    }
}

#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq, Copy, EnsureValidated)]
pub struct AbsLBConst {
    absorption_coefficient: ValidatedLBConst,
}

impl AbsLBConst {
    pub fn new(absoprtion: LinearNumberDensity) -> OpmResult<Self> {
        let mut lbc = Self::default();
        lbc.absorption_coefficient.set(absoprtion)?;
        Ok(lbc)
    }
    pub fn alpha(&self) -> LinearNumberDensity {
        *self.absorption_coefficient.get()
    }
}
