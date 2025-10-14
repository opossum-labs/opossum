use crate::{error::{OpmResult, OpossumError}, generic_validators::Validate};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsNormal;

impl Validate<f64> for IsNormal{
    fn validate(&self, value: &f64) -> OpmResult<()> {
        if (*value).is_normal() { Ok(()) } else { Err(OpossumError::Other("Value must be normal".into())) }
    }
}
impl Validate<Length> for IsNormal{
    fn validate(&self, value: &Length) -> OpmResult<()> {
        if (*value).is_normal() { Ok(()) } else { Err(OpossumError::Other("Value must be normal".into())) }
    }
}