//! Validation rules for property values.
use approx::abs_diff_ne;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use uom::si::{angle::degree, f64::Angle};

use crate::{
    coatings::CoatingType,
    error::{OpmResult, OpossumError},
    nodes::ideal_filter::FilterTypeBuilder,
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
    /// The optic shape must fit the component: open only where it may be, and with a bore exactly
    /// where the component has one.
    OpticShape {
        /// Whether the optic shape may be open, as on a detector, a dummy or an ideal surface.
        may_be_open: bool,
        /// Whether the component has a bore, as a bored mirror; every other component has none.
        bored: bool,
    },
    /// The coating of a surface must be valid (see [`CoatingType::is_valid`]) and fit the surface:
    /// a separator on a splitting surface, any other coating on every other surface (see
    /// [`SurfacePorts::splitting`](crate::core_optics::SurfacePorts::splitting)).
    SurfaceCoating {
        /// Whether the surface splits the light.
        splitting: bool,
    },
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
            Self::OpticShape { may_be_open, bored } => {
                Self::validate_optic_shape(prop, *may_be_open, *bored)
            }
            Self::SurfaceCoating { splitting } => Self::validate_surface_coating(prop, *splitting),
            Self::OrValidator { validators } => Self::validate_or_validator(prop, validators),
            Self::AndValidator { validators } => Self::validate_and_validator(prop, validators),
            Self::LightDataBuilderValidator => Self::validate_light_data_builder(prop),
        }
    }
    /// Return whether a value of the kind of `prop` can pass this validator, judged by its kind
    /// alone and not by its contents.
    ///
    /// An editor that offers a choice of kinds (such as an open optic shape) asks here, so it
    /// offers only what the property can take; the value itself is still checked by
    /// [`Validator::validate`] when it is set. Only [`Validator::OpticShape`], by whether an optic
    /// shape is open and whether it has a bore, and [`Validator::SurfaceCoating`] restrict kinds.
    ///
    /// # Arguments
    ///
    /// * `prop` - a value of the kind in question; only its kind is looked at.
    #[must_use]
    pub fn may_accept(&self, prop: &Proptype) -> bool {
        match (self, prop) {
            (Self::OpticShape { may_be_open, bored }, Proptype::OpticShape(optic_shape)) => {
                optic_shape
                    .outline()
                    .map_or(*may_be_open, |outline| outline.bore().is_some() == *bored)
            }
            (Self::SurfaceCoating { splitting }, Proptype::SurfaceConfig(config)) => {
                matches!(config.coating(), CoatingType::Separator(_)) == *splitting
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
                | Self::OpticShape { .. }
                | Self::SurfaceCoating { .. }
                | Self::LightDataBuilderValidator,
                _,
            ) => true,
        }
    }
    fn validate_surface_coating(prop: &Proptype, splitting: bool) -> OpmResult<()> {
        let Proptype::SurfaceConfig(config) = prop else {
            return Ok(());
        };
        let coating = config.coating();
        if matches!(coating, CoatingType::Separator(_)) != splitting {
            return Err(OpossumError::Properties(format!(
                "the coating '{coating}' does not fit this surface: a splitting surface takes only \
                 a separator coating, every other surface any coating but a separator"
            )));
        }
        if !coating.is_valid()? {
            return Err(OpossumError::Properties(format!(
                "the coating {coating:?} is outside its allowed range"
            )));
        }
        Ok(())
    }
    fn validate_optic_shape(prop: &Proptype, may_be_open: bool, bored: bool) -> OpmResult<()> {
        let Proptype::OpticShape(optic_shape) = prop else {
            return Ok(());
        };
        match optic_shape.outline() {
            None if !may_be_open => Err(OpossumError::Properties(
                "a component ends somewhere, so its optic shape cannot be open".into(),
            )),
            Some(outline) if outline.bore().is_some() != bored => {
                Err(OpossumError::Properties(if bored {
                    "a bored mirror has a bore through it, so its optic shape needs one".into()
                } else {
                    "only a bored mirror takes a bore through its optic shape".into()
                }))
            }
            None | Some(_) => Ok(()),
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
    use crate::{
        apertures::{Bore, CircleShape, OpticShape, Outline},
        degree, millimeter,
    };

    /// An optic shape is open only where it may be, and has a bore exactly where the component
    /// has one; the kinds of optic shapes are judged alike by `may_accept` and `validate`. No
    /// other validator restricts the kinds of optic shapes.
    #[test]
    fn an_optic_shape_is_open_and_bored_only_where_it_may_be() -> crate::error::OpmResult<()> {
        let open = Proptype::OpticShape(OpticShape::Open);
        let plain = Proptype::OpticShape(CircleShape::new(millimeter!(5.0))?.into());
        let bored = Proptype::OpticShape(
            Outline::new(
                CircleShape::new(millimeter!(5.0))?,
                Some(Bore::new(
                    CircleShape::new(millimeter!(1.0))?,
                    millimeter!(0.0, 0.0),
                    degree!(0.0),
                )),
            )?
            .into(),
        );
        for ((may_be_open, has_bore), [takes_open, takes_plain, takes_bored]) in [
            ((false, false), [false, true, false]),
            ((true, false), [true, true, false]),
            ((false, true), [false, false, true]),
            ((true, true), [true, false, true]),
        ] {
            let validator = Validator::OpticShape {
                may_be_open,
                bored: has_bore,
            };
            for (shape, takes) in [
                (&open, takes_open),
                (&plain, takes_plain),
                (&bored, takes_bored),
            ] {
                assert_eq!(
                    validator.may_accept(shape),
                    takes,
                    "{validator:?}, {shape:?}"
                );
                assert_eq!(
                    validator.validate(shape).is_ok(),
                    takes,
                    "{validator:?}, {shape:?}"
                );
            }
        }
        assert!(
            Validator::NumericIsPositive.may_accept(&open),
            "a validator of numbers does not restrict the kinds of optic shapes"
        );
        Ok(())
    }
    /// A combined validator admits a kind if any of its members does (or) or all of them do (and).
    #[test]
    fn may_accept_combines_its_members() {
        let open = Proptype::OpticShape(OpticShape::Open);
        let bounded = Validator::OpticShape {
            may_be_open: false,
            bored: false,
        };
        let members = vec![bounded, Validator::NumericIsPositive];
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

#[cfg(test)]
mod test_surface_coating {
    use super::*;
    use crate::{coatings::SplittingConfigBuilder, core_optics::SurfaceConfig};

    fn surface(coating: CoatingType) -> Proptype {
        SurfaceConfig::from(coating).into()
    }
    /// A splitting surface takes a separator and nothing else, every other surface anything but a
    /// separator; the kind alone decides, for both the editor's offer and the check.
    #[test]
    fn a_surface_takes_a_separator_exactly_when_it_splits() {
        let separator = surface(SplittingConfigBuilder::default().into());
        let antireflective = surface(CoatingType::IdealAR);
        for splitting in [true, false] {
            let validator = Validator::SurfaceCoating { splitting };
            assert_eq!(validator.may_accept(&separator), splitting);
            assert_eq!(validator.validate(&separator).is_ok(), splitting);
            assert_eq!(validator.may_accept(&antireflective), !splitting);
            assert_eq!(validator.validate(&antireflective).is_ok(), !splitting);
        }
    }
    /// A separator on a splitting surface must transmit within zero and one, although it is of the
    /// right kind.
    #[test]
    fn a_separator_out_of_range_is_refused() {
        let validator = Validator::SurfaceCoating { splitting: true };
        let too_much = surface(SplittingConfigBuilder::FixedRatio(1.5).into());
        assert!(validator.may_accept(&too_much));
        assert!(validator.validate(&too_much).is_err());
    }
}
