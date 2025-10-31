use std::ops::Range;

use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{Target, Validate, ValidateVec, numlike::NumLike},
};
use nalgebra::Point2;
use opm_macros_lib::ValidateNumeric;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    sign_positive,
    message = "All value must be positive!",
    target = "both",
    mode = "all"
)]
pub struct AllPositive;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    sign_positive,
    message = "X-value must be positive!",
    target = "x",
    mode = "all"
)]
pub struct XPositive;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    sign_positive,
    message = "Y-value must be positive!",
    target = "y",
    mode = "all"
)]
pub struct YPositive;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::Validate;
    use nalgebra::Point2;
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Length};
    use uom::si::length::meter;

    #[test]
    fn test_is_positive_i32() {
        let validator = AllPositive;
        assert!(validator.validate(&10_i32).is_ok());
        assert!(validator.validate(&-1_i32).is_err());
        assert!(validator.validate(&0_i32).is_ok());
    }

    #[test]
    fn test_is_positive_f64() {
        let validator = AllPositive;
        assert!(validator.validate(&3.14).is_ok());
        assert!(validator.validate(&-2.7).is_err());
        assert!(validator.validate(&0.0).is_ok());
    }

    #[test]
    fn test_is_positive_length() {
        let validator = AllPositive;
        assert!(validator.validate(&Length::new::<meter>(1.0)).is_ok());
        assert!(validator.validate(&Length::new::<meter>(-1.0)).is_err());
        assert!(validator.validate(&Length::new::<meter>(0.0)).is_ok());
    }

    #[test]
    fn test_is_positive_angle() {
        let validator = AllPositive;
        assert!(validator.validate(&Angle::new::<radian>(2.0)).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(-0.5)).is_err());
        assert!(validator.validate(&Angle::new::<radian>(0.0)).is_ok());
    }

    #[test]
    fn test_is_positive_point2_f64() {
        let validator = AllPositive;
        let p_valid = Point2::new(1.0, 2.0);
        let p_invalid_x = Point2::new(0.0, 2.0);
        let p_invalid_y = Point2::new(1.0, -1.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_ok());
        assert!(validator.validate(&p_invalid_y).is_err());
    }

    #[test]
    fn test_is_positive_point2_length() {
        let validator = AllPositive;
        let p_valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_invalid_x = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(2.0));
        let p_invalid_y = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(-1.0));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_ok());
        assert!(validator.validate(&p_invalid_y).is_err());
    }

    #[test]
    fn test_is_positive_tuple_length() {
        let validator = AllPositive;
        let p_valid = (Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_invalid_x = (Length::new::<meter>(0.0), Length::new::<meter>(2.0));
        let p_invalid_y = (Length::new::<meter>(1.0), Length::new::<meter>(-1.0));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_ok());
        assert!(validator.validate(&p_invalid_y).is_err());
    }

    #[test]
    fn test_is_positive_tuple_f64() {
        let validator = AllPositive;
        let p_valid = (1.0, 2.0);
        let p_invalid_x = (0.0, 2.0);
        let p_invalid_y = (1.0, -1.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_ok());
        assert!(validator.validate(&p_invalid_y).is_err());
    }

    // -----------------------------
    // XPositive Tests
    // -----------------------------
    #[test]
    fn test_x_positive_i32() {
        let validator = XPositive;
        assert!(validator.validate(&10_i32).is_ok());
        assert!(validator.validate(&-1_i32).is_err());
        assert!(validator.validate(&0_i32).is_ok());
    }

    #[test]
    fn test_x_positive_f64() {
        let validator = XPositive;
        assert!(validator.validate(&3.14).is_ok());
        assert!(validator.validate(&-2.7).is_err());
        assert!(validator.validate(&0.0).is_ok());
    }

    #[test]
    fn test_x_positive_point2_f64() {
        let validator = XPositive;
        let p_valid = Point2::new(1.0, -100.0);
        let p_invalid = Point2::new(-1.0, 50.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_x_positive_point2_length() {
        let validator = XPositive;
        let p_valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(-5.0));
        let p_invalid = Point2::new(Length::new::<meter>(-1.0), Length::new::<meter>(2.0));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    // -----------------------------
    // YPositive Tests
    // -----------------------------
    #[test]
    fn test_y_positive_i32() {
        let validator = YPositive;
        assert!(validator.validate(&10_i32).is_ok());
        assert!(validator.validate(&-1_i32).is_err());
        assert!(validator.validate(&0_i32).is_ok());
    }

    #[test]
    fn test_y_positive_f64() {
        let validator = YPositive;
        assert!(validator.validate(&3.14).is_ok());
        assert!(validator.validate(&-2.7).is_err());
        assert!(validator.validate(&0.0).is_ok());
    }

    #[test]
    fn test_y_positive_point2_f64() {
        let validator = YPositive;
        let p_valid = Point2::new(-10.0, 5.0);
        let p_invalid = Point2::new(20.0, -1.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_y_positive_point2_length() {
        let validator = YPositive;
        let p_valid = Point2::new(Length::new::<meter>(-1.0), Length::new::<meter>(2.0));
        let p_invalid = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(-3.0));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }
}
