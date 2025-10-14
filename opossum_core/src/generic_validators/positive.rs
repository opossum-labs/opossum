
use num_traits::float::FloatCore;
use uom::si::f64::Length;
use crate::{error::{OpmResult, OpossumError}, generic_validators::Validate};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsPositive;
impl Validate<f64> for IsPositive{
    fn validate(&self, value: &f64) -> OpmResult<()> {
        if (*value).is_sign_positive() {Ok(())  } else { Err(OpossumError::Other("Value must be positive".into())) }
    }
}

impl Validate<Length> for IsPositive{
    fn validate(&self, value: &Length) -> OpmResult<()> {
        if (*value).is_sign_positive() {Ok(())  } else { Err(OpossumError::Other("Value must be positive".into())) }
    }
}
