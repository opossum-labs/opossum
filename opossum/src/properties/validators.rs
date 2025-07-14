//! A collection of common validators for the property system.
use crate::error::{OpmResult, OpossumError};
use crate::properties::{proptype::Proptype, validator::Validator};

#[derive(Debug, Clone)]
pub struct F64IsFinite;

impl Validator for F64IsFinite {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        if let Proptype::F64(v) = val {
            if v.is_finite() {
                Ok(())
            } else {
                Err(OpossumError::Properties(format!(
                    "Validation failed: value {v} must be finite (not NaN or +/- Infinity)"
                )))
            }
        } else {
            // Silently ignore types that are not F64, matching original logic.
            Ok(())
        }
    }
}

/// Returns a validator that checks if an `f64` value is finite (not NaN or +/- Infinity).
#[must_use]
pub fn f64_is_finite() -> Box<dyn Validator> {
    Box::new(F64IsFinite)
}

#[derive(Debug, Clone)]
pub struct F64IsPositive;

impl Validator for F64IsPositive {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        if let Proptype::F64(v) = val {
            if *v > 0.0 {
                Ok(())
            } else {
                Err(OpossumError::Properties(format!(
                    "Validation failed: value {v} must be positive."
                )))
            }
        } else {
            // Silently ignore types that are not F64, matching original logic.
            Ok(())
        }
    }
}

/// Returns a validator that checks if an `f64` value is positive (> 0.0).
#[must_use]
pub fn f64_is_positive() -> Box<dyn Validator> {
    Box::new(F64IsPositive)
}

#[derive(Debug, Clone)]
pub struct F64InRange {
    min: f64,
    max: f64,
}

impl Validator for F64InRange {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        if let Proptype::F64(v) = val {
            if *v >= self.min && *v <= self.max {
                Ok(())
            } else {
                Err(OpossumError::Properties(format!(
                    "Validation failed: value {} is outside the allowed range [{}, {}].",
                    v, self.min, self.max
                )))
            }
        } else {
            Ok(())
        }
    }
}

/// Returns a validator that checks if an `f64` value is within a given range (inclusive).
#[must_use]
pub fn f64_in_range(min: f64, max: f64) -> Box<dyn Validator> {
    Box::new(F64InRange { min, max })
}

#[derive(Debug, Clone)]
pub struct StringIsNotEmpty;

impl Validator for StringIsNotEmpty {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        if let Proptype::String(s) = val {
            if s.is_empty() {
                Err(OpossumError::Properties(
                    "Validation failed: string must not be empty.".to_string(),
                ))
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }
}

/// Returns a validator that checks if a `String` value is not empty.
#[must_use]
pub fn string_not_empty() -> Box<dyn Validator> {
    Box::new(StringIsNotEmpty)
}

#[derive(Debug, Clone)]
pub struct OrValidator {
    validators: Vec<Box<dyn Validator>>,
}

impl Validator for OrValidator {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        let mut result = Ok(());
        for validator in &self.validators {
            result = validator.validate(val);
            if result.is_ok() {
                break;
            }
        }
        result
    }
}

/// Returns a validator that checks if the `or` combination of a set of validators is valid.
#[must_use]
pub fn or_validator(validators: Vec<Box<dyn Validator>>) -> Box<dyn Validator> {
    Box::new(OrValidator { validators })
}

#[derive(Debug, Clone)]
pub struct AndValidator {
    validators: Vec<Box<dyn Validator>>,
}

impl Validator for AndValidator {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        for validator in &self.validators {
            validator.validate(val)?;
        }
        Ok(())
    }
}

/// Returns a validator that checks if the `and` combination of a set of validators is valid.
#[must_use]
pub fn and_validator(validators: Vec<Box<dyn Validator>>) -> Box<dyn Validator> {
    Box::new(AndValidator { validators })
}
