//! A collection of common validators for the property system.
use crate::error::{OpmResult, OpossumError};
use crate::properties::{proptype::Proptype, validator::Validator};

#[derive(Debug, Clone, Copy)]
pub struct F64IsPositive;

impl Validator for F64IsPositive {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        if let Proptype::F64(v) = val {
            if *v > 0.0 {
                Ok(())
            } else {
                Err(OpossumError::Properties(format!(
                    "Validation failed: value {} must be positive.",
                    v
                )))
            }
        } else {
            // Silently ignore types that are not F64, matching original logic.
            Ok(())
        }
    }
}

/// Returns a validator that checks if an `f64` value is positive (> 0.0).
pub fn f64_is_positive() -> Box<dyn Validator> {
    Box::new(F64IsPositive)
}

#[derive(Debug, Clone, Copy)]
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
pub fn f64_in_range(min: f64, max: f64) -> Box<dyn Validator> {
    Box::new(F64InRange { min, max })
}

#[derive(Debug, Clone, Copy)]
pub struct StringIsNotEmpty;

impl Validator for StringIsNotEmpty {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        if let Proptype::String(s) = val {
            if !s.is_empty() {
                Ok(())
            } else {
                Err(OpossumError::Properties(
                    "Validation failed: string must not be empty.".to_string(),
                ))
            }
        } else {
            Ok(())
        }
    }
}

/// Returns a validator that checks if a `String` value is not empty.
pub fn string_not_empty() -> Box<dyn Validator> {
    Box::new(StringIsNotEmpty)
}

// --- END: MODIFIED CODE ---
