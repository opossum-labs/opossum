//! A collection of common validators for the property system.
use uom::si::angle::degree;
use uom::si::f64::Angle;

use crate::error::{OpmResult, OpossumError};
use crate::properties::{proptype::Proptype, validator::Validator};

#[derive(Debug, Clone)]
struct NumericIsFinite;

impl Validator for NumericIsFinite {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        let is_valid = match val {
            Proptype::F64(v) => v.is_finite(),
            Proptype::Angle(a) => a.is_finite(),
            Proptype::Energy(e) => e.is_finite(),
            Proptype::Fluence(f) => f.is_finite(),
            Proptype::Length(l) => l.is_finite(),
            Proptype::LinearDensity(d) => d.is_finite(),
            Proptype::WfLambda(l, _) => l.is_finite(),
            _ => true, // Silently ignore if not numeric or I32 which is always finite
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "value {val:?} must be finite (not NaN or +/- Infinity)"
            )))
        }
    }
}

/// Returns a validator that checks if a numeric value is finite (not NaN or +/- Infinity).
#[must_use]
pub fn numeric_is_finite() -> Box<dyn Validator> {
    Box::new(NumericIsFinite)
}

#[derive(Debug, Clone)]
struct NumericIsNotNaN;

impl Validator for NumericIsNotNaN {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        let is_valid = match val {
            Proptype::F64(v) => !v.is_nan(),
            Proptype::Angle(a) => !a.is_nan(),
            Proptype::Energy(e) => !e.is_nan(),
            Proptype::Fluence(f) => !f.is_nan(),
            Proptype::Length(l) => !l.is_nan(),
            Proptype::LinearDensity(d) => !d.is_nan(),
            Proptype::WfLambda(l, _) => !l.is_nan(),
            _ => true, // Silently ignore if not numeric or I32 which is always not NaN
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "value {val:?} must not be NaN"
            )))
        }
    }
}

/// Returns a validator that checks if a numeric value is not NaN.
#[must_use]
pub fn numeric_is_not_nan() -> Box<dyn Validator> {
    Box::new(NumericIsNotNaN)
}

#[derive(Debug, Clone)]
struct NumericIsNotZero;

impl Validator for NumericIsNotZero {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        let is_valid = match val {
            Proptype::F64(v) => *v != 0.0,
            Proptype::Angle(a) => a.value != 0.0,
            Proptype::Energy(e) => e.value != 0.0,
            Proptype::Fluence(f) => f.value != 0.0,
            Proptype::I32(i) => *i != 0,
            Proptype::Length(l) => l.value != 0.0,
            Proptype::LinearDensity(d) => d.value != 0.0,
            Proptype::WfLambda(l, _) => *l != 0.0,
            _ => true, // Silently ignore if not numeric
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "value {val:?} must not be zero"
            )))
        }
    }
}

/// Returns a validator that checks if a numeric value is not NaN.
#[must_use]
pub fn numeric_is_not_zero() -> Box<dyn Validator> {
    Box::new(NumericIsNotZero)
}

#[derive(Debug, Clone)]
struct NumericIsPositive;

impl Validator for NumericIsPositive {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        let is_valid = match val {
            Proptype::F64(v) => v.is_sign_positive(),
            Proptype::Angle(a) => a.is_sign_positive(),
            Proptype::Energy(e) => e.is_sign_positive(),
            Proptype::Fluence(f) => f.is_sign_positive(),
            Proptype::I32(i) => i.is_positive(),
            Proptype::Length(l) => l.is_sign_positive(),
            Proptype::LinearDensity(d) => d.is_sign_positive(),
            Proptype::WfLambda(l, _) => l.is_sign_positive(),
            _ => true, // Silently ignore if not numeric
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "value {val:?} must be positive."
            )))
        }
    }
}

/// Returns a validator that checks if an `f64` value is positive (> 0.0).
#[must_use]
pub fn numeric_is_positive() -> Box<dyn Validator> {
    Box::new(NumericIsPositive)
}

#[derive(Debug, Clone)]
struct NumericInRange {
    min: f64,
    max: f64,
}

impl Validator for NumericInRange {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        let allowed_range = self.min..=self.max;
        let is_valid = match val {
            Proptype::F64(v) => allowed_range.contains(v),
            Proptype::Angle(a) => allowed_range.contains(&a.value),
            Proptype::Energy(e) => allowed_range.contains(&e.value),
            Proptype::Fluence(f) => allowed_range.contains(&f.value),
            Proptype::I32(i) => allowed_range.contains(&f64::from(*i)),
            Proptype::Length(l) => allowed_range.contains(&l.value),
            Proptype::LinearDensity(d) => allowed_range.contains(&d.value),
            Proptype::WfLambda(l, _) => allowed_range.contains(l),
            _ => true, // Silently ignore if not numeric
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "value {:?} is outside the allowed range [{}, {}].",
                val, self.min, self.max
            )))
        }
    }
}

/// Returns a validator that checks if an `f64` value is within a given range (inclusive).
#[must_use]
pub fn numeric_in_range(min: f64, max: f64) -> Box<dyn Validator> {
    Box::new(NumericInRange { min, max })
}

#[derive(Debug, Clone)]
struct AngleInRange {
    min: Angle,
    max: Angle,
    inclusive: bool,
}

impl Validator for AngleInRange {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        let is_valid = if self.inclusive {
            match val {
                Proptype::Angle(a) => (self.min..=self.max).contains(a),
                _ => true, // Silently ignore if not numeric
            }
        } else {
            match val {
                Proptype::Angle(a) => a > &self.min && a < &self.max,
                _ => true, // Silently ignore if not numeric
            }
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "angle {:?} is outside the allowed range [{}°, {}°].",
                val,
                self.min.get::<degree>(),
                self.max.get::<degree>()
            )))
        }
    }
}

/// Returns a validator that checks if an `f64` value is within a given range (inclusive).
#[must_use]
pub fn angle_in_range(min: Angle, max: Angle, inclusive: bool) -> Box<dyn Validator> {
    Box::new(AngleInRange {
        min,
        max,
        inclusive,
    })
}

#[derive(Debug, Clone)]
struct StringIsNotEmpty;

impl Validator for StringIsNotEmpty {
    fn validate(&self, val: &Proptype) -> OpmResult<()> {
        if let Proptype::String(s) = val {
            if s.is_empty() {
                Err(OpossumError::Properties(
                    "string must not be empty.".to_string(),
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
struct OrValidator {
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
struct AndValidator {
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
