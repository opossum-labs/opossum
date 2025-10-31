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
    message = "All value must be normal!",
    target = "both",
    mode = "all"
)]
pub struct AllNormal;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    normal,
    message = "X-value must be normal!",
    target = "x",
    mode = "all"
)]
pub struct XNormal;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default, ValidateNumeric)]
#[rule(
    normal,
    message = "Y-value must be normal!",
    target = "y",
    mode = "all"
)]
pub struct YNormal;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::Validate;
    use nalgebra::Point2;
    use uom::si::angle::radian;
    use uom::si::f64::{Angle, Length};
    use uom::si::length::meter;

    #[test]
    fn test_all_normal_f64() {
        let validator = AllNormal;

        assert!(validator.validate(&1.0).is_ok());
        assert!(validator.validate(&0.0).is_err());
        assert!(validator.validate(&-42.0).is_ok());

        assert!(validator.validate(&f64::INFINITY).is_err());
        assert!(validator.validate(&f64::NEG_INFINITY).is_err());
        assert!(validator.validate(&f64::NAN).is_err());
    }

    #[test]
    fn test_x_normal_point2() {
        let validator = XNormal;

        let p_valid = Point2::new(1.0, f64::INFINITY);
        let p_invalid = Point2::new(f64::NAN, 5.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_y_normal_point2() {
        let validator = YNormal;

        let p_valid = Point2::new(f64::INFINITY, 2.0);
        let p_invalid = Point2::new(1.0, f64::NAN);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_all_normal_point2() {
        let validator = AllNormal;

        let p_valid = Point2::new(1.0, 2.0);
        let p_invalid_x = Point2::new(f64::NAN, 2.0);
        let p_invalid_y = Point2::new(1.0, f64::INFINITY);
        let p_invalid_both = Point2::new(f64::NAN, f64::INFINITY);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_err());
        assert!(validator.validate(&p_invalid_y).is_err());
        assert!(validator.validate(&p_invalid_both).is_err());
    }

    #[test]
    fn test_vec_all_normal() {
        let validator = AllNormal;

        let v_valid = vec![1.0, 2.0, 3.0];
        let v_invalid = vec![1.0, f64::NAN, 3.0];

        assert!(validator.validate_vec(&v_valid).is_ok());
        assert!(validator.validate_vec(&v_invalid).is_err());
    }

    #[test]
    fn test_vec_point2_xnormal() {
        let validator = XNormal;

        let v_valid = vec![Point2::new(1.0, f64::INFINITY), Point2::new(1.0, -42.0)];
        let v_invalid = vec![Point2::new(f64::NAN, 5.0), Point2::new(1.0, 2.0)];

        assert!(validator.validate_vec(&v_valid).is_ok());
        assert!(validator.validate_vec(&v_invalid).is_err());
    }

    #[test]
    fn test_vec_point2_ynormal() {
        let validator = YNormal;

        let v_valid = vec![Point2::new(f64::INFINITY, 1.0), Point2::new(-42.0, 1.0)];
        let v_invalid = vec![Point2::new(1.0, f64::NAN), Point2::new(2.0, 3.0)];

        assert!(validator.validate_vec(&v_valid).is_ok());
        assert!(validator.validate_vec(&v_invalid).is_err());
    }

    #[test]
    fn test_is_normal_f64() {
        let validator = AllNormal;

        // normal values
        assert!(validator.validate(&1.0).is_ok());
        assert!(validator.validate(&-1.0).is_ok());
        assert!(validator.validate(&f64::MIN_POSITIVE).is_ok());

        // not normal values
        assert!(validator.validate(&0.0).is_err());
        assert!(validator.validate(&-0.0).is_err());
        assert!(validator.validate(&f64::NAN).is_err());
        assert!(validator.validate(&f64::INFINITY).is_err());
        assert!(validator.validate(&f64::NEG_INFINITY).is_err());
        assert!(validator.validate(&(f64::MIN_POSITIVE / 2.0)).is_err()); // subnormal
    }

    #[test]
    fn test_is_normal_length() {
        let validator = AllNormal;

        let l_valid = Length::new::<meter>(1.0);
        let l_zero = Length::new::<meter>(0.0);
        let l_subnormal = Length::new::<meter>(f64::MIN_POSITIVE / 2.0);
        let l_inf = Length::new::<meter>(f64::INFINITY);

        assert!(validator.validate(&l_valid).is_ok());
        assert!(validator.validate(&l_zero).is_err());
        assert!(validator.validate(&l_subnormal).is_err());
        assert!(validator.validate(&l_inf).is_err());
        assert!(validator.validate(&Length::new::<meter>(f64::NAN)).is_err());
    }

    #[test]
    fn test_is_normal_point2_f64() {
        let validator = AllNormal;

        let p_valid = Point2::new(1.0, 2.0);
        let p_invalid_zero = Point2::new(0.0, 1.0);
        let p_invalid_subnormal = Point2::new(f64::MIN_POSITIVE / 2.0, 1.0);
        let p_invalid_nan = Point2::new(1.0, f64::NAN);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_zero).is_err());
        assert!(validator.validate(&p_invalid_subnormal).is_err());
        assert!(validator.validate(&p_invalid_nan).is_err());
    }

    #[test]
    fn test_is_normal_point2_length() {
        let validator = AllNormal;

        let p_valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_invalid_zero = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(1.0));
        let p_invalid_subnormal = Point2::new(
            Length::new::<meter>(f64::MIN_POSITIVE / 2.0),
            Length::new::<meter>(1.0),
        );
        let p_invalid_nan = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(f64::NAN));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_zero).is_err());
        assert!(validator.validate(&p_invalid_subnormal).is_err());
        assert!(validator.validate(&p_invalid_nan).is_err());
    }

    #[test]
    fn test_is_normal_tuple_f64() {
        let validator = AllNormal;

        let p_valid = (1.0, 2.0);
        let p_invalid_zero = (0.0, 1.0);
        let p_invalid_subnormal = (f64::MIN_POSITIVE / 2.0, 1.0);
        let p_invalid_nan = (1.0, f64::NAN);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_zero).is_err());
        assert!(validator.validate(&p_invalid_subnormal).is_err());
        assert!(validator.validate(&p_invalid_nan).is_err());
    }

    #[test]
    fn test_is_normal_tuple_length() {
        let validator = AllNormal;

        let p_valid = (Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_invalid_zero = (Length::new::<meter>(0.0), Length::new::<meter>(1.0));
        let p_invalid_subnormal = (
            Length::new::<meter>(f64::MIN_POSITIVE / 2.0),
            Length::new::<meter>(1.0),
        );
        let p_invalid_nan = (Length::new::<meter>(1.0), Length::new::<meter>(f64::NAN));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_zero).is_err());
        assert!(validator.validate(&p_invalid_subnormal).is_err());
        assert!(validator.validate(&p_invalid_nan).is_err());
    }

    #[test]
    fn test_is_normal_angle() {
        let validator = AllNormal;

        let a_valid = Angle::new::<radian>(1.0);
        let a_valid_neg = Angle::new::<radian>(-1.0);
        let a_zero = Angle::new::<radian>(0.0);
        let a_subnormal = Angle::new::<radian>(f64::MIN_POSITIVE / 2.0);
        let a_inf = Angle::new::<radian>(f64::INFINITY);
        let a_nan = Angle::new::<radian>(f64::NAN);

        // normal values
        assert!(validator.validate(&a_valid).is_ok());
        assert!(validator.validate(&a_valid_neg).is_ok());
        assert!(
            validator
                .validate(&Angle::new::<radian>(f64::MIN_POSITIVE))
                .is_ok()
        );

        // not normal values
        assert!(validator.validate(&a_zero).is_err());
        assert!(validator.validate(&a_subnormal).is_err());
        assert!(validator.validate(&a_inf).is_err());
        assert!(validator.validate(&a_nan).is_err());
    }
}
