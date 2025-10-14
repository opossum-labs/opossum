use num_traits::float::FloatCore;
use uom::si::f64::Length;

use crate::{error::{OpmResult, OpossumError}, generic_validators::Validate};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsFinite;
impl <T:FloatCore>Validate<T> for IsFinite{
    fn validate(&self, value: &T) -> OpmResult<()> {
        if !(*value).is_finite() { Ok(()) } else { Err(OpossumError::Other("Value must be finite".into())) }
    }
}