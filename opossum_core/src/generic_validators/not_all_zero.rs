use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{Target, Validate, ValidateVec, numlike::NumLike},
};
use nalgebra::Point2;
use opm_macros_lib::ValidateNumeric;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    normal,
    message = "At least one value must be non-zero!",
    target = "both",
    mode = "any"
)]
pub struct NotAllZero;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    normal,
    message = "At least one x-value must be normal!",
    target = "x",
    mode = "any"
)]
pub struct XNotAllZero;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    normal,
    message = "At least one y-value must be normal!",
    target = "y",
    mode = "any"
)]
pub struct YNotAllZero;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::Validate;
    use nalgebra::Point2;

    #[test]
    fn test_not_all_zero_f64() {
        let validator = NotAllZero;

        let all_zero = [0.0, 0.0, 0.0];
        let one_non_zero = [0.0, 1.0, 0.0];

        assert!(validator.validate_vec(&one_non_zero).is_ok());
        assert!(validator.validate_vec(&all_zero).is_err());
    }

    #[test]
    fn test_not_all_zero_point2() {
        let validator = NotAllZero;

        let p_all_zero = Point2::new(0.0, 0.0);
        let p_x_nonzero = Point2::new(1.0, 0.0);
        let p_y_nonzero = Point2::new(0.0, -5.0);

        assert!(validator.validate(&p_all_zero).is_err());
        assert!(validator.validate(&p_x_nonzero).is_ok());
        assert!(validator.validate(&p_y_nonzero).is_ok());
    }

    #[test]
    fn test_not_all_zero_vec_point2() {
        let validator = NotAllZero;

        let v_all_zero = vec![Point2::new(0.0, 0.0), Point2::new(0.0, 0.0)];
        let v_one_non_zero = vec![Point2::new(0.0, 0.0), Point2::new(2.0, 0.0)];

        assert!(validator.validate_vec(&v_all_zero).is_err());
        assert!(validator.validate_vec(&v_one_non_zero).is_ok());
    }

    #[test]
    fn test_x_not_all_zero_point2() {
        let validator = XNotAllZero;

        let p_all_zero = Point2::new(0.0, 5.0);
        let p_x_nonzero = Point2::new(1.0, 0.0);

        assert!(validator.validate(&p_all_zero).is_err());
        assert!(validator.validate(&p_x_nonzero).is_ok());
    }

    #[test]
    fn test_y_not_all_zero_point2() {
        let validator = YNotAllZero;

        let p_all_zero = Point2::new(3.0, 0.0);
        let p_y_nonzero = Point2::new(0.0, 1.0);

        assert!(validator.validate(&p_all_zero).is_err());
        assert!(validator.validate(&p_y_nonzero).is_ok());
    }

    #[test]
    fn test_x_not_all_zero_vec_point2() {
        let validator = XNotAllZero;

        let v_all_zero = vec![Point2::new(0.0, 1.0), Point2::new(0.0, 2.0)];
        let v_one_non_zero = vec![Point2::new(0.0, 1.0), Point2::new(3.0, 2.0)];

        assert!(validator.validate_vec(&v_all_zero).is_err());
        assert!(validator.validate_vec(&v_one_non_zero).is_ok());
    }

    #[test]
    fn test_y_not_all_zero_vec_point2() {
        let validator = YNotAllZero;

        let v_all_zero = vec![Point2::new(1.0, 0.0), Point2::new(2.0, 0.0)];
        let v_one_non_zero = vec![Point2::new(1.0, 0.0), Point2::new(2.0, 4.0)];

        assert!(validator.validate_vec(&v_all_zero).is_err());
        assert!(validator.validate_vec(&v_one_non_zero).is_ok());
    }
}
