use crate::impl_validator;
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Length};

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct AllNotNaN;

impl_validator!(AllNotNaN, |_self, v: &f64| !v.is_nan(), f64);
impl_validator!(AllNotNaN, |_self, v: &Length| !v.is_nan(), Length);
impl_validator!(AllNotNaN, |_self, v: &Angle| !v.is_nan(), Angle);
impl_validator!(
    AllNotNaN,
    |_self, v: &Point2<f64>| !v.x.is_nan() && !v.y.is_nan(),
    Point2<f64>
);
impl_validator!(
    AllNotNaN,
    |_self, v: &Point2<Length>| !v.x.is_nan() && !v.y.is_nan(),
    Point2<Length>
);

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
        let validator = AllNotNaN;

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
        let validator = AllNotNaN;

        let l_valid = Length::new::<meter>(0.0);
        let l_nan = Length::new::<meter>(f64::NAN);

        assert!(validator.validate(&l_valid).is_ok());
        assert!(validator.validate(&Length::new::<meter>(1.0)).is_ok());
        assert!(validator.validate(&l_nan).is_err());
    }

    #[test]
    fn test_is_not_nan_angle() {
        let validator = AllNotNaN;

        let a_valid = Angle::new::<radian>(0.0);
        let a_nan = Angle::new::<radian>(f64::NAN);

        assert!(validator.validate(&a_valid).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(1.0)).is_ok());
        assert!(validator.validate(&a_nan).is_err());
    }

    #[test]
    fn test_is_not_nan_point2_f64() {
        let validator = AllNotNaN;

        let p_valid = Point2::new(0.0, 1.0);
        let p_invalid_x = Point2::new(f64::NAN, 1.0);
        let p_invalid_y = Point2::new(1.0, f64::NAN);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_err());
        assert!(validator.validate(&p_invalid_y).is_err());
    }

    #[test]
    fn test_is_not_nan_point2_length() {
        let validator = AllNotNaN;

        let p_valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_invalid_x = Point2::new(Length::new::<meter>(f64::NAN), Length::new::<meter>(2.0));
        let p_invalid_y = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(f64::NAN));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid_x).is_err());
        assert!(validator.validate(&p_invalid_y).is_err());
    }
}
