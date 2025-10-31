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
    not_zero,
    message = "All value must be non-zero!",
    target = "both",
    mode = "all"
)]
pub struct AllNotZero;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    not_zero,
    message = "X-value must be non-zero!",
    target = "x",
    mode = "all"
)]
#[allow(dead_code)]
pub struct XNotZero;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    not_zero,
    message = "Y-value must be non-zero!",
    target = "y",
    mode = "all"
)]
#[allow(dead_code)]
pub struct YNotZero;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::Validate;
    use nalgebra::Point2;
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Length};
    use uom::si::length::meter;

    #[test]
    fn test_all_not_zero_i32() {
        let validator = AllNotZero;
        assert!(validator.validate(&1_i32).is_ok());
        assert!(validator.validate(&-1_i32).is_ok());
        assert!(validator.validate(&0_i32).is_err());
    }

    #[test]
    fn test_all_not_zero_f64() {
        let validator = AllNotZero;
        assert!(validator.validate(&1.5_f64).is_ok());
        assert!(validator.validate(&-3.2_f64).is_ok());
        assert!(validator.validate(&0.0_f64).is_err());
    }

    #[test]
    fn test_all_not_zero_length() {
        let validator = AllNotZero;
        assert!(validator.validate(&Length::new::<meter>(1.0)).is_ok());
        assert!(validator.validate(&Length::new::<meter>(-2.0)).is_ok());
        assert!(validator.validate(&Length::new::<meter>(0.0)).is_err());
    }

    #[test]
    fn test_all_not_zero_angle() {
        let validator = AllNotZero;
        assert!(validator.validate(&Angle::new::<radian>(1.0)).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(-1.0)).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(0.0)).is_err());
    }

    #[test]
    fn test_all_not_zero_point2_f64() {
        let validator = AllNotZero;
        let valid = Point2::new(1.0, -2.0);
        let invalid_x = Point2::new(0.0, 2.0);
        let invalid_y = Point2::new(1.0, 0.0);

        assert!(validator.validate(&valid).is_ok());
        assert!(validator.validate(&invalid_x).is_err());
        assert!(validator.validate(&invalid_y).is_err());
    }

    #[test]
    fn test_all_not_zero_point2_length() {
        let validator = AllNotZero;
        let valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let invalid_x = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(1.0));
        let invalid_y = Point2::new(Length::new::<meter>(2.0), Length::new::<meter>(0.0));

        assert!(validator.validate(&valid).is_ok());
        assert!(validator.validate(&invalid_x).is_err());
        assert!(validator.validate(&invalid_y).is_err());
    }

    #[test]
    fn test_x_not_zero_point2_f64() {
        let validator = XNotZero;
        let valid = Point2::new(1.0, 0.0);
        let invalid = Point2::new(0.0, 5.0);

        assert!(validator.validate(&valid).is_ok());
        assert!(validator.validate(&invalid).is_err());
    }

    #[test]
    fn test_x_not_zero_point2_length() {
        let validator = XNotZero;
        let valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(0.0));
        let invalid = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(5.0));

        assert!(validator.validate(&valid).is_ok());
        assert!(validator.validate(&invalid).is_err());
    }

    #[test]
    fn test_y_not_zero_point2_f64() {
        let validator = YNotZero;
        let valid = Point2::new(0.0, -2.0);
        let invalid = Point2::new(1.0, 0.0);

        assert!(validator.validate(&valid).is_ok());
        assert!(validator.validate(&invalid).is_err());
    }

    #[test]
    fn test_y_not_zero_point2_length() {
        let validator = YNotZero;
        let valid = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(1.0));
        let invalid = Point2::new(Length::new::<meter>(5.0), Length::new::<meter>(0.0));

        assert!(validator.validate(&valid).is_ok());
        assert!(validator.validate(&invalid).is_err());
    }

    #[test]
    fn test_is_not_zero_usize() {
        let validator = AllNotZero;

        assert!(validator.validate(&1_usize).is_ok());
        assert!(validator.validate(&100_usize).is_ok());
        assert!(validator.validate(&0_usize).is_err());
    }

    #[test]
    fn test_is_not_zero_i32() {
        let validator = AllNotZero;

        assert!(validator.validate(&1_i32).is_ok());
        assert!(validator.validate(&-5_i32).is_ok());
        assert!(validator.validate(&0_i32).is_err());
    }

    #[test]
    fn test_is_not_zero_f64() {
        let validator = AllNotZero;

        assert!(validator.validate(&1.0).is_ok());
        assert!(validator.validate(&-0.5).is_ok());
        assert!(validator.validate(&0.0).is_err());
    }

    #[test]
    fn test_is_not_zero_length() {
        let validator = AllNotZero;

        let l_valid = Length::new::<meter>(1.0);
        let l_zero = Length::new::<meter>(0.0);

        assert!(validator.validate(&l_valid).is_ok());
        assert!(validator.validate(&l_zero).is_err());
    }

    #[test]
    fn test_is_not_zero_angle() {
        let validator = AllNotZero;

        let a_valid = Angle::new::<radian>(1.0);
        let a_zero = Angle::new::<radian>(0.0);

        assert!(validator.validate(&a_valid).is_ok());
        assert!(validator.validate(&a_zero).is_err());
    }

    #[test]
    fn test_is_not_zero_point2_f64() {
        let validator = AllNotZero;

        let p_valid = Point2::new(1.0, 2.0);
        let p_zero_x = Point2::new(0.0, 2.0);
        let p_zero_y = Point2::new(1.0, 0.0);
        let p_zero_both = Point2::new(0.0, 0.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_zero_x).is_err());
        assert!(validator.validate(&p_zero_y).is_err());
        assert!(validator.validate(&p_zero_both).is_err());
    }

    #[test]
    fn test_is_not_zero_point2_length() {
        let validator = AllNotZero;

        let p_valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_zero_x = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(2.0));
        let p_zero_y = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(0.0));
        let p_zero_both = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(0.0));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_zero_x).is_err());
        assert!(validator.validate(&p_zero_y).is_err());
        assert!(validator.validate(&p_zero_both).is_err());
    }

    #[test]
    fn test_is_not_zero_tuple_f64() {
        let validator = AllNotZero;

        let p_valid = (1.0, 2.0);
        let p_zero_x = (0.0, 2.0);
        let p_zero_y = (1.0, 0.0);
        let p_zero_both = (0.0, 0.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_zero_x).is_err());
        assert!(validator.validate(&p_zero_y).is_err());
        assert!(validator.validate(&p_zero_both).is_err());
    }

    #[test]
    fn test_is_not_zero_tuple_length() {
        let validator = AllNotZero;

        let p_valid = (Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_zero_x = (Length::new::<meter>(0.0), Length::new::<meter>(2.0));
        let p_zero_y = (Length::new::<meter>(1.0), Length::new::<meter>(0.0));
        let p_zero_both = (Length::new::<meter>(0.0), Length::new::<meter>(0.0));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_zero_x).is_err());
        assert!(validator.validate(&p_zero_y).is_err());
        assert!(validator.validate(&p_zero_both).is_err());
    }
}
