use crate::{error::OpmResult, generic_validators::Validate};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct OrValidator<T:Clone, V1: Validate<T>, V2: Validate<T>> {
    v1: V1,
    v2: V2,
    _marker: PhantomData<T>,
}

impl<T:Clone, V1, V2> Validate<T> for OrValidator<T, V1, V2>
where
    V1: Validate<T>,
    V2: Validate<T>,
{
    fn validate(&self, value: &T) -> OpmResult<()> {
        self.v1.validate(value).or_else(|_| self.v2.validate(value))
    }
}

impl<T:Clone, V1: Validate<T>, V2: Validate<T>> OrValidator<T, V1, V2> {
    pub const fn new(v1: V1, v2: V2) -> Self {
        Self {
            v1,
            v2,
            _marker: PhantomData,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct AndValidator<T:Clone, V1: Validate<T>, V2: Validate<T>> {
    v1: V1,
    v2: V2,
    _marker: PhantomData<T>,
}

impl<T:Clone, V1, V2> Validate<T> for AndValidator<T, V1, V2>
where
    V1: Validate<T>,
    V2: Validate<T>,
{
    fn validate(&self, value: &T) -> OpmResult<()> {
        self.v1.validate(value)?;
        self.v2.validate(value)
    }
}

impl<T:Clone, V1: Validate<T>, V2: Validate<T>> AndValidator<T, V1, V2> {
    pub const fn new(v1: V1, v2: V2) -> Self {
        Self {
            v1,
            v2,
            _marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::generic_validators::{
        AndValidator, IsFinite, IsNotZero, IsPositive, OrValidator, Validate,
    };
    use nalgebra::Point2;

    #[test]
    fn test_or_validator_f64() {
        let validator = OrValidator::new(IsPositive, IsNotZero);

        // Positive and non-zero
        assert!(validator.validate(&5.0).is_ok());

        // Negative but non-zero
        assert!(validator.validate(&-2.0).is_ok());

        // Zero and non-positive
        assert!(validator.validate(&0.0).is_ok());

        assert!(validator.validate(&-0.0).is_err());
    }

    #[test]
    fn test_and_validator_f64() {
        let validator = AndValidator::new(IsPositive, IsFinite);

        // Positive and finite
        assert!(validator.validate(&5.0).is_ok());

        // Negative
        assert!(validator.validate(&-2.0).is_err());

        // Infinite
        assert!(validator.validate(&f64::INFINITY).is_err());
        assert!(validator.validate(&f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn test_or_validator_point2() {
        let validator = OrValidator::new(IsFinite, IsPositive);

        let p1 = Point2::new(1.0, -2.0); // finite but one negative
        let p2 = Point2::new(f64::INFINITY, 5.0); // not finite but positive
        let p3 = Point2::new(f64::NAN, -1.0); // neither

        assert!(validator.validate(&p1).is_ok());
        assert!(validator.validate(&p2).is_ok());
        assert!(validator.validate(&p3).is_err());
    }

    #[test]
    fn test_and_validator_point2() {
        let validator = AndValidator::new(IsPositive, IsFinite);

        let p1 = Point2::new(2.0, 3.0); // positive and finite
        let p2 = Point2::new(2.0, -1.0); // one negative
        let p3 = Point2::new(f64::INFINITY, 1.0); // one infinite

        assert!(validator.validate(&p1).is_ok());
        assert!(validator.validate(&p2).is_err());
        assert!(validator.validate(&p3).is_err());
    }

    #[test]
    fn test_nested_validator_f64() {
        // Nested validator: A && (B || C) && D
        let inner_or = OrValidator::new(IsFinite, IsNotZero);
        let outer_and = AndValidator::new(IsPositive, inner_or);
        let full_validator = AndValidator::new(outer_and, IsNotZero);

        // Should pass: positive, finite, not zero
        assert!(full_validator.validate(&5.0).is_ok());

        // Fail: negative value
        assert!(full_validator.validate(&-2.0).is_err());

        // Fail: zero value
        assert!(full_validator.validate(&0.0).is_err());

        // Pass: positive, not finite (Inf), not zero
        assert!(full_validator.validate(&f64::INFINITY).is_ok());
    }
}
