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
    finite,
    message = "All value must be finite!",
    target = "both",
    mode = "all"
)]
pub struct AllFinite;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, ValidateNumeric)]
#[rule(
    finite,
    message = "X-value must be finite!",
    target = "x",
    mode = "all"
)]
pub struct XFinite;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, ValidateNumeric)]
#[rule(
    finite,
    message = "Y-value must be finite!",
    target = "y",
    mode = "all"
)]
pub struct YFinite;

#[cfg(test)]
mod tests {
    use crate::generic_validators::Validate;

    use super::*;
    use nalgebra::Point2;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    #[test]
    fn test_is_finite_f64() {
        let validator = AllFinite;

        assert!(validator.validate(&1.23).is_ok());
        assert!(validator.validate(&0.0).is_ok());
        assert!(validator.validate(&f64::INFINITY).is_err());
        assert!(validator.validate(&f64::NEG_INFINITY).is_err());
        assert!(validator.validate(&f64::NAN).is_err());
    }

    #[test]
    fn test_is_finite_length() {
        let validator = AllFinite;
        let l = Length::new::<meter>(1.0);
        let l_inf = Length::new::<meter>(f64::INFINITY);

        assert!(validator.validate(&l).is_ok());
        assert!(validator.validate(&Length::new::<meter>(0.0)).is_ok());
        assert!(validator.validate(&l_inf).is_err());
        assert!(validator.validate(&Length::new::<meter>(f64::NAN)).is_err());
    }

    #[test]
    fn test_is_finite_point2_f64() {
        let validator = AllFinite;

        let p = Point2::new(1.0, 2.0);
        let p_inf = Point2::new(f64::INFINITY, 0.0);
        let p_nan = Point2::new(0.0, f64::NAN);

        assert!(validator.validate(&p).is_ok());
        assert!(validator.validate(&p_inf).is_err());
        assert!(validator.validate(&p_nan).is_err());
    }

    #[test]
    fn test_is_finite_point2_length() {
        let validator = AllFinite;

        let p = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_inf = Point2::new(
            Length::new::<meter>(f64::INFINITY),
            Length::new::<meter>(0.0),
        );
        let p_nan = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(f64::NAN));

        assert!(validator.validate(&p).is_ok());
        assert!(validator.validate(&p_inf).is_err());
        assert!(validator.validate(&p_nan).is_err());
    }

    #[test]
    fn test_is_finite_tuple_f64() {
        let validator = AllFinite;

        let p = (1.0, 2.0);
        let p_inf = (f64::INFINITY, 0.0);
        let p_nan = (0.0, f64::NAN);

        assert!(validator.validate(&p).is_ok());
        assert!(validator.validate(&p_inf).is_err());
        assert!(validator.validate(&p_nan).is_err());
    }

    #[test]
    fn test_is_finite_tuple_length() {
        let validator = AllFinite;

        let p = (Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_inf = (
            Length::new::<meter>(f64::INFINITY),
            Length::new::<meter>(0.0),
        );
        let p_nan = (Length::new::<meter>(0.0), Length::new::<meter>(f64::NAN));

        assert!(validator.validate(&p).is_ok());
        assert!(validator.validate(&p_inf).is_err());
        assert!(validator.validate(&p_nan).is_err());
    }

    #[test]
    fn test_all_finite_f64() {
        let validator = AllFinite;

        assert!(validator.validate(&1.0).is_ok());
        assert!(validator.validate(&0.0).is_ok());
        assert!(validator.validate(&-42.0).is_ok());

        assert!(validator.validate(&f64::INFINITY).is_err());
        assert!(validator.validate(&f64::NEG_INFINITY).is_err());
        assert!(validator.validate(&f64::NAN).is_err());
    }

    #[test]
    fn test_x_finite_point2() {
        let validator = XFinite;

        let p_valid = Point2::new(1.0, f64::INFINITY);
        let p_invalid = Point2::new(f64::NAN, 5.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_y_finite_point2() {
        let validator = YFinite;

        let p_valid = Point2::new(f64::INFINITY, 2.0);
        let p_invalid = Point2::new(1.0, f64::NAN);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_all_finite_point2() {
        let validator = AllFinite;

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
    fn test_vec_all_finite() {
        let validator = AllFinite;

        let v_valid = vec![1.0, 2.0, 3.0];
        let v_invalid = vec![1.0, f64::NAN, 3.0];

        assert!(validator.validate_vec(&v_valid).is_ok());
        assert!(validator.validate_vec(&v_invalid).is_err());
    }

    #[test]
    fn test_vec_point2_xfinite() {
        let validator = XFinite;

        let v_valid = vec![Point2::new(1.0, f64::INFINITY), Point2::new(0.0, -42.0)];
        let v_invalid = vec![Point2::new(f64::NAN, 5.0), Point2::new(1.0, 2.0)];

        assert!(validator.validate_vec(&v_valid).is_ok());
        assert!(validator.validate_vec(&v_invalid).is_err());
    }

    #[test]
    fn test_vec_point2_yfinite() {
        let validator = YFinite;

        let v_valid = vec![Point2::new(f64::INFINITY, 1.0), Point2::new(-42.0, 0.0)];
        let v_invalid = vec![Point2::new(1.0, f64::NAN), Point2::new(2.0, 3.0)];

        assert!(validator.validate_vec(&v_valid).is_ok());
        assert!(validator.validate_vec(&v_invalid).is_err());
    }
}
