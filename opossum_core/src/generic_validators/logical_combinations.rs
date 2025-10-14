use std::marker::PhantomData;
use crate::{error::OpmResult, generic_validators::{IsNormal, IsPositive, Validate}};
use serde::{Deserialize, Serialize};




#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct OrValidator<T, V1:Validate<T>, V2:Validate<T>> {
    v1: V1,
    v2: V2,
    _marker: PhantomData<T>,
}


impl<T, V1, V2> Validate<T> for OrValidator<T, V1, V2> 
    where
    V1: Validate<T>,
    V2: Validate<T>,{
    fn validate(&self, value: &T) -> OpmResult<()> {
        self.v1.validate(value).or_else(|_| self.v2.validate(value))
    }
}


impl<T, V1: Validate<T>, V2: Validate<T>>  OrValidator<T, V1, V2> {
    pub fn new(v1: V1, v2: V2) -> Self {
        Self { v1, v2, _marker: PhantomData }
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AndValidator<T, V1:Validate<T>, V2:Validate<T>> {
    v1: V1,
    v2: V2,
    _marker: PhantomData<T>,
}

impl<T, V1, V2> Validate<T> for AndValidator<T, V1, V2> 
    where
    V1: Validate<T>,
    V2: Validate<T>,{
    fn validate(&self, value: &T) -> OpmResult<()> {
        self.v1.validate(value)?;
        self.v2.validate(value)
    }
}


impl<T, V1: Validate<T>, V2: Validate<T>>  AndValidator<T, V1, V2> {
    pub fn new(v1: V1, v2: V2) -> Self {
        Self { v1, v2, _marker: PhantomData }
    }
}

impl<T> AndValidator<T, IsNormal, IsPositive> 
    where
    IsNormal: Validate<T>,
        IsPositive: Validate<T>,
            {
                
    pub fn new_normal_and_positive() -> Self {
        AndValidator::new(IsNormal, IsPositive)
    }
}

pub type IsNormalAndPositive<T> = AndValidator<T, IsNormal, IsPositive>;
