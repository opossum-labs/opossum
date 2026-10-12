use approx::abs_diff_ne;
use serde::{Deserialize, Serialize};
use uom::si::{angle::degree, f64::Angle};

use crate::{
    error::{OpmResult, OpossumError},
    nodes::{SplittingConfigBuilder, ideal_filter::FilterTypeBuilder},
    properties::Proptype,
};
use std::fmt::Debug;

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
    /// The aperture shape must delimit a region, i.e. have a hard edge.
    ApertureDelimitsRegion,
    OrValidator {
        validators: Vec<Self>,
    },
    AndValidator {
        validators: Vec<Self>,
    },
    LightDataBuilderValidator,
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
            Self::ApertureDelimitsRegion => Self::validate_aperture_delimits_region(prop),
            Self::OrValidator { validators } => Self::validate_or_validator(prop, validators),
            Self::AndValidator { validators } => Self::validate_and_validator(prop, validators),
            Self::LightDataBuilderValidator => Self::validate_light_data_builder(prop),
        }
    }
    fn validate_aperture_delimits_region(prop: &Proptype) -> OpmResult<()> {
        let Proptype::Aperture(shape) = prop else {
            // Silently ignore if not an aperture.
            return Ok(());
        };
        if shape.is_binary() {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "an aperture of shape '{shape}' does not delimit a region: only a shape with a \
                 hard edge can state where something begins and ends"
            )))
        }
    }
    fn validate_numeric_is_finite(prop: &Proptype) -> OpmResult<()> {
        let is_valid = match prop {
            Proptype::F64(v) => v.is_finite(),
            Proptype::Angle(a) => a.is_finite(),
            Proptype::Energy(e) => e.is_finite(),
            Proptype::Fluence(f) => f.is_finite(),
            Proptype::Length(l) | Proptype::Curvature(l) => l.is_finite(),
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
            Proptype::Length(l) | Proptype::Curvature(l) => !l.is_nan(),
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
            Proptype::F64(v) => abs_diff_ne!(*v, 0.0),
            Proptype::Angle(a) => abs_diff_ne!(a.value, 0.0),
            Proptype::Energy(e) => abs_diff_ne!(e.value, 0.0),
            Proptype::Fluence(f) => abs_diff_ne!(f.value, 0.0),
            Proptype::I32(i) => *i != 0,
            Proptype::Length(l) | Proptype::Curvature(l) => abs_diff_ne!(l.value, 0.0),
            Proptype::LinearDensity(d) => abs_diff_ne!(d.value, 0.0),
            Proptype::WfLambda(l, _) => abs_diff_ne!(*l, 0.0),
            _ => true,
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
        // Enforce strictly positive values (> 0.0) consistent with integer semantics
        let is_valid = match prop {
            Proptype::F64(v) => *v > 0.0,
            Proptype::Angle(a) => a.value > 0.0,
            Proptype::Energy(e) => e.value > 0.0,
            Proptype::Fluence(f) => f.value > 0.0,
            Proptype::I32(i) => i.is_positive(),
            Proptype::Length(l) | Proptype::Curvature(l) => l.value > 0.0,
            Proptype::LinearDensity(d) => d.value > 0.0,
            Proptype::WfLambda(l, _) => *l > 0.0,
            _ => true,
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
            Proptype::Length(l) | Proptype::Curvature(l) => allowed_range.contains(&l.value),
            Proptype::LinearDensity(d) => allowed_range.contains(&d.value),
            Proptype::WfLambda(l, _) => allowed_range.contains(l),
            Proptype::FilterTypeBuilder(ftb) => match ftb {
                FilterTypeBuilder::Constant(c) => allowed_range.contains(&c.transmission().value),
                FilterTypeBuilder::Spectrum(s) => s.values_are_in_range(min, max)?,
            },
            Proptype::SplittingConfigBuilder(ftb) => match ftb {
                SplittingConfigBuilder::FixedRatio(c) => allowed_range.contains(c),
                SplittingConfigBuilder::Spectrum(s) => s.values_are_in_range(min, max)?,
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
    fn validate_or_validator(prop: &Proptype, validators: &[Self]) -> OpmResult<()> {
        if validators.is_empty() {
            return Err(OpossumError::Properties(
                "empty OR validator cannot match".to_string(),
            ));
        }

        let mut last_err = None;
        for validator in validators {
            match validator.validate(prop) {
                Ok(()) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            OpossumError::Properties("all validation branches failed".to_string())
        }))
    }
    fn validate_and_validator(prop: &Proptype, validators: &[Self]) -> OpmResult<()> {
        for validator in validators {
            validator.validate(prop)?;
        }
        Ok(())
    }
    fn validate_light_data_builder(prop: &Proptype) -> OpmResult<()> {
        if let Proptype::LightDataBuilder(ldb) = prop {
            match ldb.clone().build() {
                Err(e) => Err(OpossumError::Properties(format!(
                    "light_data_builder validation failed: {e}"
                ))),
                Ok(_) => Ok(()),
            }
        } else {
            Err(OpossumError::Properties(
                "lightdatabuilder validator is only for Proptype::LightDataBuilder".into(),
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;
    use uom::si::{angle::degree, f64::Angle};

    #[test]
    fn validate_finite_and_nan() {
        let val_finite = Validator::NumericIsFinite;
        let val_nan = Validator::NumericIsNotNaN;

        assert!(val_finite.validate(&Proptype::F64(42.0)).is_ok());
        assert!(val_finite.validate(&Proptype::F64(f64::INFINITY)).is_err());
        assert!(val_finite.validate(&Proptype::F64(f64::NAN)).is_err());

        assert!(val_nan.validate(&Proptype::F64(f64::NAN)).is_err());
        assert!(val_nan.validate(&Proptype::F64(f64::INFINITY)).is_ok());
    }

    #[test]
    fn validate_positive() {
        let val = Validator::NumericIsPositive;
        assert!(val.validate(&Proptype::F64(1.0)).is_ok());
        assert!(val.validate(&Proptype::F64(0.0)).is_err());
        assert!(val.validate(&Proptype::F64(-1.0)).is_err());

        assert!(val.validate(&Proptype::I32(5)).is_ok());
        assert!(val.validate(&Proptype::I32(0)).is_err());
        assert!(val.validate(&Proptype::I32(-2)).is_err());

        assert!(val.validate(&Proptype::Length(meter!(0.005))).is_ok());
        assert!(val.validate(&Proptype::Length(meter!(0.0))).is_err());
    }

    #[test]
    fn validate_not_zero() {
        let val = Validator::NumericIsNotZero;
        assert!(val.validate(&Proptype::F64(0.0)).is_err());
        assert!(val.validate(&Proptype::F64(1e-12)).is_ok());
        assert!(val.validate(&Proptype::I32(0)).is_err());
        assert!(val.validate(&Proptype::I32(1)).is_ok());
    }

    #[test]
    fn validate_numeric_in_range() {
        let val = Validator::NumericInRange {
            min: -10.0,
            max: 10.0,
        };
        assert!(val.validate(&Proptype::F64(-10.0)).is_ok());
        assert!(val.validate(&Proptype::F64(0.0)).is_ok());
        assert!(val.validate(&Proptype::F64(10.0)).is_ok());
        assert!(val.validate(&Proptype::F64(10.01)).is_err());
        assert!(val.validate(&Proptype::F64(-10.01)).is_err());
    }

    #[test]
    fn validate_angle_in_range() {
        let val_inclusive = Validator::AngleInRange {
            min: Angle::new::<degree>(0.0),
            max: Angle::new::<degree>(90.0),
            inclusive: true,
        };
        let val_exclusive = Validator::AngleInRange {
            min: Angle::new::<degree>(0.0),
            max: Angle::new::<degree>(90.0),
            inclusive: false,
        };

        let angle_0 = Proptype::Angle(Angle::new::<degree>(0.0));
        let angle_45 = Proptype::Angle(Angle::new::<degree>(45.0));
        let angle_90 = Proptype::Angle(Angle::new::<degree>(90.0));

        assert!(val_inclusive.validate(&angle_0).is_ok());
        assert!(val_inclusive.validate(&angle_45).is_ok());
        assert!(val_inclusive.validate(&angle_90).is_ok());

        assert!(val_exclusive.validate(&angle_0).is_err());
        assert!(val_exclusive.validate(&angle_45).is_ok());
        assert!(val_exclusive.validate(&angle_90).is_err());
    }

    #[test]
    fn validate_string_not_empty() {
        let val = Validator::StringIsNotEmpty;
        assert!(val.validate(&Proptype::String("data".to_string())).is_ok());
        assert!(val.validate(&Proptype::String(String::new())).is_err());
        // Non-string variants are ignored
        assert!(val.validate(&Proptype::I32(0)).is_ok());
    }

    #[test]
    fn validate_composite_and_or() {
        let val_and = Validator::AndValidator {
            validators: vec![Validator::NumericIsNotZero, Validator::NumericIsPositive],
        };
        assert!(val_and.validate(&Proptype::F64(1.0)).is_ok());
        assert!(val_and.validate(&Proptype::F64(0.0)).is_err());
        assert!(val_and.validate(&Proptype::F64(-1.0)).is_err());

        let val_or = Validator::OrValidator {
            validators: vec![
                Validator::NumericInRange { min: 0.0, max: 1.0 },
                Validator::NumericInRange {
                    min: 10.0,
                    max: 20.0,
                },
            ],
        };
        assert!(val_or.validate(&Proptype::F64(0.5)).is_ok());
        assert!(val_or.validate(&Proptype::F64(15.0)).is_ok());
        assert!(val_or.validate(&Proptype::F64(5.0)).is_err());

        let empty_or = Validator::OrValidator { validators: vec![] };
        assert!(empty_or.validate(&Proptype::F64(1.0)).is_err());
    }
}
