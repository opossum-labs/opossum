use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{Target, Validate, ValidateVec, numlike::NumLike},
};
use nalgebra::Point2;
use opm_macros_lib::ValidateNumeric;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, ValidateNumeric)]
#[rule(
    not_nan,
    message = "All value must not be nan!",
    target = "both",
    mode = "all"
)]
#[allow(dead_code)]
pub struct AllNotNan;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, ValidateNumeric)]
#[rule(
    not_nan,
    message = "X-value must not be nan!",
    target = "x",
    mode = "all"
)]
#[allow(dead_code)]
pub struct XNotNan;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, ValidateNumeric)]
#[rule(
    not_nan,
    message = "Y-value must not be nan!",
    target = "y",
    mode = "all"
)]
#[allow(dead_code)]
pub struct YNotNan;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::Validate;
    use nalgebra::Point2;
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Length};
    use uom::si::length::meter;

    #[test]
    fn test_is_not_nan_f64() {
        let validator = AllNotNan;

        // valid values
        assert!(validator.validate(&0.0).is_ok());
        assert!(validator.validate(&1.0).is_ok());
        assert!(validator.validate(&-1.0).is_ok());
        assert!(validator.validate(&(f64::MIN_POSITIVE / 2.0)).is_ok());
        assert!(validator.validate(&f64::INFINITY).is_ok());
        assert!(validator.validate(&f64::NEG_INFINITY).is_ok());

        // invalid value
        assert!(validator.validate(&f64::NAN).is_err());
    }

    #[test]
    fn test_is_not_nan_length() {
        let validator = AllNotNan;

        let l_valid = Length::new::<meter>(0.0);
        let l_nan = Length::new::<meter>(f64::NAN);

        assert!(validator.validate(&l_valid).is_ok());
        assert!(validator.validate(&Length::new::<meter>(1.0)).is_ok());
        assert!(validator.validate(&l_nan).is_err());
    }

    #[test]
    fn test_is_not_nan_angle() {
        let validator = AllNotNan;

        let a_valid = Angle::new::<radian>(0.0);
        let a_nan = Angle::new::<radian>(f64::NAN);

        assert!(validator.validate(&a_valid).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(1.0)).is_ok());
        assert!(validator.validate(&a_nan).is_err());
    }

    #[test]
    fn test_is_not_nan_point2_f64() {
        let validator = AllNotNan;

        let p_valid = Point2::new(0.0, 1.0);
        let p_invalid_x = Point2::new(f64::NAN, 1.0);
        let p_invalid_y = Point2::new(1.0, f64::NAN);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_err());
        assert!(validator.validate(&p_invalid_y).is_err());
    }

    #[test]
    fn test_is_not_nan_point2_length() {
        let validator = AllNotNan;

        let p_valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_invalid_x = Point2::new(Length::new::<meter>(f64::NAN), Length::new::<meter>(2.0));
        let p_invalid_y = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(f64::NAN));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_err());
        assert!(validator.validate(&p_invalid_y).is_err());
    }
}
