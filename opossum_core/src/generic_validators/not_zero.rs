use num::Num;
use crate::{error::{OpmResult, OpossumError}, generic_validators::Validate};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct NotZero;
impl <T: Num>Validate<T> for NotZero{
    fn validate(&self, value: &T) -> OpmResult<()> {
        if (*value).is_zero() { Err(OpossumError::Other("Value must be non-zero".into())) } else { Ok(()) }
    }
}
