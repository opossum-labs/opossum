use num_traits::float::FloatCore;

use crate::{error::{OpmResult, OpossumError}, generic_validators::Validate};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsNotNaN;

impl <T: FloatCore>Validate<T> for IsNotNaN{
    fn validate(&self, value: &T) -> OpmResult<()> {
        if (*value).is_nan() { Err(OpossumError::Other("Value must not be NaN".into())) } else { Ok(()) }
    }
}
