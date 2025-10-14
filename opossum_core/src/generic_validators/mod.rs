use crate::error::OpmResult;

mod finite;
mod not_empty;
mod not_zero;
mod logical_combinations;
mod in_range;
mod positive;
mod normal;
mod not_nan;

pub use not_zero::NotZero;
pub use finite::IsFinite;
pub use normal::IsNormal;
pub use logical_combinations::{ IsNormalAndPositive, AndValidator};
pub use positive::IsPositive;
use serde::{Deserialize, Serialize};
pub trait Validate<T> {
    fn validate(&self, value: &T) -> OpmResult<()>;
}


#[derive(Copy, Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Validated<T, V: Validate<T>> {
    value: T,
    validator: V,
}

impl<T, V: Validate<T>> Validated<T, V> {
    pub fn new(value: T, validator: V) -> OpmResult<Self> {
        validator.validate(&value)?;
        Ok(Self { value, validator})
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn set(&mut self, new_value: T) -> OpmResult<()> {
        self.validator.validate(&new_value)?;
        self.value = new_value;
        Ok(())
    }

    pub fn into_inner(self) -> T {
        self.value
    }
}

