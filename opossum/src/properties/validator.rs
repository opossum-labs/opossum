use serde::{Deserialize, Serialize};
use uom::si::{angle::degree, f64::Angle};

use crate::{
    error::{OpmResult, OpossumError},
    nodes::{SplittingConfigBuilder, ideal_filter::FilterTypeBuilder},
    properties::Proptype,
};
use std::fmt::Debug;

// pub trait DynClone {
//     fn dyn_clone(&self) -> Box<dyn Validator>;
// }
// impl<T> DynClone for T
// where
//     T: 'static + Validator + Clone,
// {
//     fn dyn_clone(&self) -> Box<dyn Validator> {
//         Box::new(self.clone())
//     }
// }
// // Implement Clone for Box<dyn Validator>
// impl Clone for Box<dyn Validator> {
//     fn clone(&self) -> Self {
//         self.dyn_clone()
//     }
// }
// pub trait Validator: DynClone + Debug + Send + Sync {
//     /// Validate a given `Proptype`.
//     ///
//     /// # Errors
//     ///
//     /// This function will return an error if the validation was not successful.
//     fn validate(&self, prop: &Proptype) -> OpmResult<()>;
// }

// impl<F> Validator for F
// where
//     F: Fn(&Proptype) -> OpmResult<()> + Clone + Debug + Send + Sync + 'static,
// {
//     fn validate(&self, prop: &Proptype) -> OpmResult<()> {
//         // Just call the closure.
//         self(prop)
//     }
// }

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Validator {
    NumericIsFinite,
    NumericIsNotNaN,
    NumericIsNotZero,
    NumericIsPositive,
    NumericInRange {
        min: f64,
        max: f64,
    },
    AngleInRange {
        min: Angle,
        max: Angle,
        inclusive: bool,
    },
    StringIsNotEmpty,
    OrValidator {
        validators: Vec<Self>,
    },
    AndValidator {
        validators: Vec<Self>,
    },
}

impl Validator {
    /// Validate a given `Proptype`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the validation was not successful.
    pub fn validate(&self, prop: &Proptype) -> OpmResult<()> {
        match self {
            Self::NumericIsFinite => Self::validate_numeric_is_finite(prop),
            Self::NumericIsNotNaN => Self::validate_numeric_is_not_nan(prop),
            Self::NumericIsNotZero => Self::validate_numeric_is_not_zero(prop),
            Self::NumericIsPositive => Self::validate_numeric_is_positive(prop),
            Self::NumericInRange { min, max } => {
                Self::validate_numeric_is_in_range(prop, *min, *max)
            }
            Self::AngleInRange {
                min,
                max,
                inclusive,
            } => Self::validate_angle_is_in_range(prop, *min, *max, *inclusive),
            Self::StringIsNotEmpty => Self::validate_string_is_not_empty(prop),
            Self::OrValidator { validators } => Self::validate_or_validator(prop, validators),
            Self::AndValidator { validators } => Self::validate_and_validator(prop, validators),
        }
    }
    fn validate_numeric_is_finite(prop: &Proptype) -> OpmResult<()> {
        let is_valid = match prop {
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
                "value {prop:?} must be finite (not NaN or +/- Infinity)"
            )))
        }
    }
    fn validate_numeric_is_not_nan(prop: &Proptype) -> OpmResult<()> {
        let is_valid = match prop {
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
                "value {prop:?} must not be NaN"
            )))
        }
    }
    fn validate_numeric_is_not_zero(prop: &Proptype) -> OpmResult<()> {
        let is_valid = match prop {
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
                "value {prop:?} must not be zero"
            )))
        }
    }

    fn validate_numeric_is_positive(prop: &Proptype) -> OpmResult<()> {
        let is_valid = match prop {
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
                "value {prop:?} must be positive."
            )))
        }
    }

    fn validate_numeric_is_in_range(prop: &Proptype, min: f64, max: f64) -> OpmResult<()> {
        let allowed_range = min..=max;
        let is_valid = match prop {
            Proptype::F64(v) => allowed_range.contains(v),
            Proptype::Angle(a) => allowed_range.contains(&a.value),
            Proptype::Energy(e) => allowed_range.contains(&e.value),
            Proptype::Fluence(f) => allowed_range.contains(&f.value),
            Proptype::I32(i) => allowed_range.contains(&f64::from(*i)),
            Proptype::Length(l) => allowed_range.contains(&l.value),
            Proptype::LinearDensity(d) => allowed_range.contains(&d.value),
            Proptype::WfLambda(l, _) => allowed_range.contains(l),
            Proptype::FilterTypeBuilder(ftb) => match ftb {
                FilterTypeBuilder::Constant(c) => allowed_range.contains(c),
                FilterTypeBuilder::Spectrum(s) => s.build()?.values_are_in_range(min, max),
            },
            Proptype::SplittingConfigBuilder(ftb) => match ftb {
                SplittingConfigBuilder::FixedRatio(c) => allowed_range.contains(c),
                SplittingConfigBuilder::Spectrum(s) => s.build()?.values_are_in_range(min, max),
            },
            _ => true, // Silently ignore if not numeric
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "value {prop:?} is outside the allowed range [{min}, {max}]."
            )))
        }
    }

    fn validate_angle_is_in_range(
        prop: &Proptype,
        min: Angle,
        max: Angle,
        inclusive: bool,
    ) -> OpmResult<()> {
        let is_valid = if inclusive {
            match prop {
                Proptype::Angle(a) => (min..=max).contains(a),
                _ => true, // Silently ignore if not numeric
            }
        } else {
            match prop {
                Proptype::Angle(a) => a > &min && a < &max,
                _ => true, // Silently ignore if not numeric
            }
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "angle {:?} is outside the allowed range [{}°, {}°].",
                prop,
                min.get::<degree>(),
                max.get::<degree>()
            )))
        }
    }
    fn validate_string_is_not_empty(prop: &Proptype) -> OpmResult<()> {
        if let Proptype::String(s) = prop {
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

    fn validate_or_validator(prop: &Proptype, validators: &Vec<Self>) -> OpmResult<()> {
        let mut result = Ok(());
        for validator in validators {
            result = validator.validate(prop);
            if result.is_ok() {
                break;
            }
        }
        result
    }

    fn validate_and_validator(prop: &Proptype, validators: &Vec<Self>) -> OpmResult<()> {
        for validator in validators {
            validator.validate(prop)?;
        }
        Ok(())
    }
}
