use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{Validate, ValidateVec},
};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct OrValidator<T, V1: Validate<T>, V2: Validate<T>> {
    v1: V1,
    v2: V2,
    _marker: PhantomData<T>,
}

impl<T, V1, V2> Validate<T> for OrValidator<T, V1, V2>
where
    V1: Validate<T>,
    V2: Validate<T>,
{
    fn validate(&self, value: &T) -> OpmResult<()> {
        self.v1.validate(value).or_else(|_| self.v2.validate(value))
    }
}

impl<T, V1: Validate<T>, V2: Validate<T>> OrValidator<T, V1, V2> {
    pub const fn new(v1: V1, v2: V2) -> Self {
        Self {
            v1,
            v2,
            _marker: PhantomData,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct OrValidatorVec<T, V1: ValidateVec<T>, V2: ValidateVec<T>> {
    v1: V1,
    v2: V2,
    _marker: PhantomData<T>,
}

impl<T, V1, V2> ValidateVec<T> for OrValidatorVec<T, V1, V2>
where
    V1: ValidateVec<T>,
    V2: ValidateVec<T>,
{
    fn validate_vec(&self, values: &Vec<T>) -> OpmResult<()> {
        self.v1.validate_vec(values).or_else(|_| self.v2.validate_vec(values))
    }
}

impl<T, V1: ValidateVec<T>, V2: ValidateVec<T>> OrValidatorVec<T, V1, V2> {
    pub const fn new(v1: V1, v2: V2) -> Self {
        Self {
            v1,
            v2,
            _marker: PhantomData,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct AndValidator<T, V1: Validate<T>, V2: Validate<T>> {
    v1: V1,
    v2: V2,
    _marker: PhantomData<T>,
}

impl<T, V1, V2> Validate<T> for AndValidator<T, V1, V2>
where
    V1: Validate<T>,
    V2: Validate<T>,
{
    fn validate(&self, value: &T) -> OpmResult<()> {
        self.v1.validate(value)?;
        self.v2.validate(value)
    }
}

impl<T, V1: Validate<T>, V2: Validate<T>> AndValidator<T, V1, V2> {
    pub const fn new(v1: V1, v2: V2) -> Self {
        Self {
            v1,
            v2,
            _marker: PhantomData,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct AndValidatorVec<T, V1: ValidateVec<T>, V2: ValidateVec<T>> {
    v1: V1,
    v2: V2,
    _marker: PhantomData<T>,
}

impl<T, V1, V2> ValidateVec<T> for AndValidatorVec<T, V1, V2>
where
    V1: ValidateVec<T>,
    V2: ValidateVec<T>,
{
    fn validate_vec(&self, values: &Vec<T>) -> OpmResult<()> {
        self.v1.validate_vec(values)?;
        self.v2.validate_vec(values)
    }
}

impl<T, V1: ValidateVec<T>, V2: ValidateVec<T>> AndValidatorVec<T, V1, V2> {
    pub const fn new(v1: V1, v2: V2) -> Self {
        Self {
            v1,
            v2,
            _marker: PhantomData,
        }
    }
}


/// A validator that negates the result of another validator.
///
/// # Type Parameters
/// * `T` - The type of value to validate.
/// * `V` - The inner validator to negate.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct NotValidator<T, V: Validate<T>> {
    inner: V,
    _marker: PhantomData<T>,
}

impl<T, V: Validate<T>> NotValidator<T, V> {
    /// Creates a new `NotValidator` wrapping a given validator.
    ///
    /// # Arguments
    ///
    /// * `validator` - The validator to negate.
    ///
    /// # Returns
    ///
    /// * A `NotValidator` instance.
    pub const fn new(validator: V) -> Self {
        Self {
            inner: validator,
            _marker: PhantomData,
        }
    }
}

impl<T, V: Validate<T>> Validate<T> for NotValidator<T, V> {
    /// Validates the value using the inner validator and negates its result.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to validate.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the inner validator fails.
    /// * `Err(OpossumError)` if the inner validator succeeds.
    fn validate(&self, value: &T) -> OpmResult<()> {
        match self.inner.validate(value) {
            Ok(()) => Err(OpossumError::Other(
                "Value failed NotValidator check: inner validator passed".into(),
            )),
            Err(_) => Ok(()),
        }
    }
}
/// A validator that negates the result of another validator.
///
/// # Type Parameters
/// * `T` - The type of value to validate.
/// * `V` - The inner validator to negate.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct NotValidatorVec<T, V: ValidateVec<T>> {
    inner: V,
    _marker: PhantomData<T>,
}

impl<T, V: ValidateVec<T>> NotValidatorVec<T, V> {
    /// Creates a new `NotValidator` wrapping a given validator.
    ///
    /// # Arguments
    ///
    /// * `validator` - The validator to negate.
    ///
    /// # Returns
    ///
    /// * A `NotValidator` instance.
    pub const fn new(validator: V) -> Self {
        Self {
            inner: validator,
            _marker: PhantomData,
        }
    }
}

impl<T, V: ValidateVec<T>> ValidateVec<T> for NotValidatorVec<T, V> {
    /// Validates the value using the inner validator and negates its result.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to validate.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the inner validator fails.
    /// * `Err(OpossumError)` if the inner validator succeeds.
    fn validate_vec(&self, values: &Vec<T>) -> OpmResult<()> {
        match self.inner.validate_vec(values) {
            Ok(()) => Err(OpossumError::Other(
                "Value failed NotValidator check: inner validator passed".into(),
            )),
            Err(_) => Ok(()),
        }
    }
}



#[cfg(test)]
mod tests {
    use crate::generic_validators::{
        AllFinite, AllNotZero, AllPositive, AndValidator, OrValidator, Validate,
        logical_combinations::NotValidator,
    };
    use nalgebra::Point2;

    #[test]
    fn test_or_validator_f64() {
        let validator = OrValidator::new(AllPositive, AllNotZero);

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
        let validator = AndValidator::new(AllPositive, AllFinite);

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
        let validator = OrValidator::new(AllFinite, AllPositive);

        let p1 = Point2::new(1.0, -2.0); // finite but one negative
        let p2 = Point2::new(f64::INFINITY, 5.0); // not finite but positive
        let p3 = Point2::new(f64::NAN, -1.0); // neither

        assert!(validator.validate(&p1).is_ok());
        assert!(validator.validate(&p2).is_ok());
        assert!(validator.validate(&p3).is_err());
    }

    #[test]
    fn test_and_validator_point2() {
        let validator = AndValidator::new(AllPositive, AllFinite);

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
        let inner_or = OrValidator::new(AllFinite, AllNotZero);
        let outer_and = AndValidator::new(AllPositive, inner_or);
        let full_validator = AndValidator::new(outer_and, AllNotZero);

        // Should pass: positive, finite, not zero
        assert!(full_validator.validate(&5.0).is_ok());

        // Fail: negative value
        assert!(full_validator.validate(&-2.0).is_err());

        // Fail: zero value
        assert!(full_validator.validate(&0.0).is_err());

        // Pass: positive, not finite (Inf), not zero
        assert!(full_validator.validate(&f64::INFINITY).is_ok());
    }
    #[test]
    fn test_nested_not_and_or_f64() {
        // Nested with NotValidator: !A && (B || C)
        let inner_or = OrValidator::new(AllFinite, AllNotZero);
        let not_positive = NotValidator::new(AllPositive);
        let full_validator = AndValidator::new(not_positive, inner_or);

        // Negative number, finite → passes
        assert!(full_validator.validate(&-3.5).is_ok());

        // Zero → fails because inner_or fails
        assert!(full_validator.validate(&0.0).is_err());

        // Positive number → fails because NotValidator fails
        assert!(full_validator.validate(&5.0).is_err());

        // Infinite positive → fails because NotValidator fails
        assert!(full_validator.validate(&f64::INFINITY).is_err());
    }

    #[test]
    fn test_not_validator_f64() {
        let validator = NotValidator::new(AllPositive);

        // Positive number → NotValidator should fail
        assert!(validator.validate(&5.0).is_err());

        // Zero → NotValidator should not succeed
        assert!(validator.validate(&0.0).is_err());

        // Negative number → NotValidator should succeed
        assert!(validator.validate(&-3.2).is_ok());

        // Infinite → NotValidator should succeed (since IsPositive fails)
        assert!(validator.validate(&f64::NEG_INFINITY).is_ok());
    }

    #[test]
    fn test_not_validator_is_not_zero() {
        let validator = NotValidator::new(AllNotZero);

        // Non-zero number → NotValidator should fail
        assert!(validator.validate(&5.0).is_err());
        assert!(validator.validate(&-1.0).is_err());

        // Zero → NotValidator should succeed
        assert!(validator.validate(&0.0).is_ok());
    }

    #[test]
    fn test_not_validator_point2() {
        let validator = NotValidator::new(AllNotZero);

        // Point2 with non-zero components → NotValidator fails
        assert!(validator.validate(&Point2::new(1, 2)).is_err());

        // Point2 with zero component → NotValidator succeeds
        assert!(validator.validate(&Point2::new(0, 5)).is_ok());
        assert!(validator.validate(&Point2::new(0, 0)).is_ok());
    }
}
