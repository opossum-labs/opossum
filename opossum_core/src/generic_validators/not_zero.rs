use nalgebra::Point2;
use num::Zero;
use uom::si::f64::{Angle, Length};
use crate::impl_validator;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct IsNotZero;

impl_validator!(IsNotZero, |_self, v: &usize| !v.is_zero(), usize);
impl_validator!(IsNotZero, |_self, v: &i32| !v.is_zero(), i32);
impl_validator!(IsNotZero, |_self, v: &f64| !v.is_zero(), f64);
impl_validator!(IsNotZero, |_self, v: &Length| !v.is_zero(), Length);
impl_validator!(IsNotZero, |_self, v: &Angle| !v.is_zero(), Angle);
impl_validator!(IsNotZero, |_self, v: &Point2<f64>| !v.x.is_zero() && !v.y.is_zero(), Point2<f64>);
impl_validator!(IsNotZero, |_self, v: &Point2<Length>| !v.x.is_zero() && !v.y.is_zero(), Point2<Length>);


    #[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::Validate;
    use nalgebra::Point2;
    use uom::si::f64::{Length, Angle};
    use uom::si::length::meter;
    use uom::si::angle::radian;

    #[test]
    fn test_is_not_zero_usize() {
        let validator = IsNotZero;

        assert!(validator.validate(&1_usize).is_ok());
        assert!(validator.validate(&100_usize).is_ok());
        assert!(validator.validate(&0_usize).is_err());
    }

    #[test]
    fn test_is_not_zero_i32() {
        let validator = IsNotZero;

        assert!(validator.validate(&1_i32).is_ok());
        assert!(validator.validate(&-5_i32).is_ok());
        assert!(validator.validate(&0_i32).is_err());
    }

    #[test]
    fn test_is_not_zero_f64() {
        let validator = IsNotZero;

        assert!(validator.validate(&1.0).is_ok());
        assert!(validator.validate(&-0.5).is_ok());
        assert!(validator.validate(&0.0).is_err());
    }

    #[test]
    fn test_is_not_zero_length() {
        let validator = IsNotZero;

        let l_valid = Length::new::<meter>(1.0);
        let l_zero = Length::new::<meter>(0.0);

        assert!(validator.validate(&l_valid).is_ok());
        assert!(validator.validate(&l_zero).is_err());
    }

    #[test]
    fn test_is_not_zero_angle() {
        let validator = IsNotZero;

        let a_valid = Angle::new::<radian>(1.0);
        let a_zero = Angle::new::<radian>(0.0);

        assert!(validator.validate(&a_valid).is_ok());
        assert!(validator.validate(&a_zero).is_err());
    }

    #[test]
    fn test_is_not_zero_point2_f64() {
        let validator = IsNotZero;

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
        let validator = IsNotZero;

        let p_valid = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        let p_zero_x = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(2.0));
        let p_zero_y = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(0.0));
        let p_zero_both = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(0.0));

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_zero_x).is_err());
        assert!(validator.validate(&p_zero_y).is_err());
        assert!(validator.validate(&p_zero_both).is_err());
    }
}
