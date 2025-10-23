    
use crate::{error::OpmResult, generic_validators::{Validate, ValidateVec}};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct Pass;

impl<T:Clone> Validate<T> for Pass {
    fn validate(&self, _value: &T) -> OpmResult<()> {
        Ok(())
    }
}

impl<T:Clone> ValidateVec<T> for Pass {
    fn validate_vec(&self, _values: &Vec<T>) -> OpmResult<()> {
        Ok(())
    }
}