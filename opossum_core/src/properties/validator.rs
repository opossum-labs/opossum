//! Validation rules for property values.
use approx::abs_diff_ne;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use uom::si::{angle::degree, f64::Angle};

use crate::{
    apertures::ApertureShape,
    error::{OpmResult, OpossumError},
    nodes::{SplittingConfigBuilder, ideal_filter::FilterTypeBuilder},
    properties::Proptype,
};

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
    /// The aperture shape must delimit a region or be open, i.e. bound nothing at all.
    ApertureDelimitsRegionOrIsOpen,
    OrValidator {
        validators: Vec<Self>,
    },
    AndValidator {
        validators: Vec<Self>,
    },
    LightDataBuilderValidator,
}

impl Validator {
    /// Validate a given [`Proptype`].
    ///
    /// # Errors
    /// Returns an error if validation fails.
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
            Self::ApertureDelimitsRegionOrIsOpen => {
                if matches!(prop, Proptype::Aperture(ApertureShape::Open)) {
                    Ok(())
                } else {
                    Self::validate_aperture_delimits_region(prop)
                }
            }
            Self::OrValidator { validators } => Self::validate_or_validator(prop, validators),
            Self::AndValidator { validators } => Self::validate_and_validator(prop, validators),
            Self::LightDataBuilderValidator => Self::validate_light_data_builder(prop),
        }
    }
    /// Return whether a value of the kind of `prop` can pass this validator, judged by its kind
    /// alone and not by its contents.
    ///
    /// An editor that offers a choice of kinds (such as aperture shapes) asks here, so it offers
    /// only what the property can take; the value itself is still checked by
    /// [`Validator::validate`] when it is set. Only the aperture validators restrict kinds.
    ///
    /// # Arguments
    ///
    /// * `prop` - a value of the kind in question; only its kind is looked at.
    #[must_use]
    pub fn may_accept(&self, prop: &Proptype) -> bool {
        match (self, prop) {
            (Self::ApertureDelimitsRegion, Proptype::Aperture(shape)) => shape.may_delimit_region(),
            (Self::ApertureDelimitsRegionOrIsOpen, Proptype::Aperture(shape)) => {
                matches!(shape, ApertureShape::Open) || shape.may_delimit_region()
            }
            (Self::OrValidator { validators }, _) => validators
                .iter()
                .any(|validator| validator.may_accept(prop)),
            (Self::AndValidator { validators }, _) => validators
                .iter()
                .all(|validator| validator.may_accept(prop)),
            (
                Self::NumericIsFinite
                | Self::NumericIsNotNaN
                | Self::NumericIsNotZero
                | Self::NumericIsPositive
                | Self::NumericInRange { .. }
                | Self::AngleInRange { .. }
                | Self::StringIsNotEmpty
                | Self::ApertureDelimitsRegion
                | Self::ApertureDelimitsRegionOrIsOpen
                | Self::LightDataBuilderValidator,
                _,
            ) => true,
        }
    }
    fn validate_aperture_delimits_region(prop: &Proptype) -> OpmResult<()> {
        let Proptype::Aperture(shape) = prop else {
            return Ok(());
        };
        if shape.delimits_region() {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "an aperture of shape '{shape}' does not delimit a region: only a shape with a \
                 hard edge, or a stack of such shapes holding a hole, can state where something \
                 begins and ends"
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
            _ => true,
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "Value {prop:?} must be finite (neither NaN nor Infinity)"
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
            _ => true,
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "Value {prop:?} must not be NaN"
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
                "Value {prop:?} must not be zero"
            )))
        }
    }

    fn validate_numeric_is_positive(prop: &Proptype) -> OpmResult<()> {
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
                "Value {prop:?} must be strictly positive"
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
            _ => true,
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "Value {prop:?} is outside the allowed range [{min}, {max}]"
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
                _ => true,
            }
        } else {
            match prop {
                Proptype::Angle(a) => a > &min && a < &max,
                _ => true,
            }
        };
        if is_valid {
            Ok(())
        } else {
            Err(OpossumError::Properties(format!(
                "Angle {prop:?} is outside the allowed range [{}°, {}°]",
                min.get::<degree>(),
                max.get::<degree>()
            )))
        }
    }

    fn validate_string_is_not_empty(prop: &Proptype) -> OpmResult<()> {
        if let Proptype::String(s) = prop
            && s.is_empty()
        {
            return Err(OpossumError::Properties(
                "String must not be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_or_validator(prop: &Proptype, validators: &[Self]) -> OpmResult<()> {
        if validators.is_empty() {
            return Err(OpossumError::Properties(
                "Empty OR validator cannot match".to_string(),
            ));
        }
        let mut last_error = None;
        for validator in validators {
            match validator.validate(prop) {
                Ok(()) => return Ok(()),
                Err(err) => last_error = Some(err),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            OpossumError::Properties("All validation branches failed".to_string())
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
            ldb.clone().build().map(|_| ()).map_err(|e| {
                OpossumError::Properties(format!("LightDataBuilder validation failed: {e}"))
            })
        } else {
            Err(OpossumError::Properties(
                "LightDataBuilder validator only applies to Proptype::LightDataBuilder".into(),
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

#[cfg(test)]
mod test_apertures {
    use super::*;
    use crate::apertures::StackShape;
    use strum::IntoEnumIterator;

    /// The aperture validators admit a shape by its kind: every kind that can bound a region, and
    /// `Open` where the property may also be unbounded. A default stack is refused as a value but
    /// admitted as a kind, because a stack with a hole bounds a region.
    #[test]
    fn may_accept_judges_aperture_shapes_by_kind() {
        let accepts = |validator: &Validator, shape: &ApertureShape| {
            validator.may_accept(&Proptype::Aperture(shape.clone()))
        };
        for shape in ApertureShape::iter() {
            let bounds_a_region =
                !matches!(shape, ApertureShape::Open | ApertureShape::Gaussian(_));
            let open = matches!(shape, ApertureShape::Open);
            assert_eq!(
                accepts(&Validator::ApertureDelimitsRegion, &shape),
                bounds_a_region,
                "{shape:?}"
            );
            assert_eq!(
                accepts(&Validator::ApertureDelimitsRegionOrIsOpen, &shape),
                bounds_a_region || open,
                "{shape:?}"
            );
            assert!(
                accepts(&Validator::NumericIsPositive, &shape),
                "a validator of numbers does not restrict the kinds of shapes"
            );
        }
        let default_stack = ApertureShape::Stack(StackShape::default());
        assert!(
            Validator::ApertureDelimitsRegion
                .validate(&Proptype::Aperture(default_stack.clone()))
                .is_err()
        );
        assert!(accepts(&Validator::ApertureDelimitsRegion, &default_stack));
    }
    /// A combined validator admits a kind if any of its members does (or) or all of them do (and).
    #[test]
    fn may_accept_combines_its_members() {
        let open = Proptype::Aperture(ApertureShape::Open);
        let members = vec![
            Validator::ApertureDelimitsRegion,
            Validator::ApertureDelimitsRegionOrIsOpen,
        ];
        assert!(
            Validator::OrValidator {
                validators: members.clone()
            }
            .may_accept(&open)
        );
        assert!(
            !Validator::AndValidator {
                validators: members
            }
            .may_accept(&open)
        );
    }
}
