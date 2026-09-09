use crate::impl_validator;
use nalgebra::Point2;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Length};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Default)]
pub struct OnlyOneZero;
impl_validator!(
    OnlyOneZero,
    |_self, v: &Point2<usize>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<usize>
);
impl_validator!(
    OnlyOneZero,
    |_self, v: &Point2<i32>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<i32>
);
impl_validator!(
    OnlyOneZero,
    |_self, v: &Point2<Angle>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<Angle>
);
impl_validator!(
    OnlyOneZero,
    |_self, v: &Point2<f64>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<f64>
);
impl_validator!(
    OnlyOneZero,
    |_self, v: &Point2<Length>| !(v.x.is_zero() && v.y.is_zero()),
    Point2<Length>
);

impl_validator!(
    OnlyOneZero,
    |_self, v: &(usize, usize)| !(v.0.is_zero() && v.1.is_zero()),
    (usize, usize)
);
impl_validator!(
    OnlyOneZero,
    |_self, v: &(i32, i32)| !(v.0.is_zero() && v.1.is_zero()),
    (i32, i32)
);
impl_validator!(
    OnlyOneZero,
    |_self, v: &(Angle, Angle)| !(v.0.is_zero() && v.1.is_zero()),
    (Angle, Angle)
);
impl_validator!(
    OnlyOneZero,
    |_self, v: &(f64, f64)| !(v.0.is_zero() && v.1.is_zero()),
    (f64, f64)
);
impl_validator!(
    OnlyOneZero,
    |_self, v: &(Length, Length)| !(v.0.is_zero() && v.1.is_zero()),
    (Length, Length)
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
    fn test_only_one_zero_usize() {
        let validator = OnlyOneZero;

        let p_valid1 = Point2::new(0_usize, 5_usize);
        let p_valid2 = Point2::new(3_usize, 0_usize);
        let p_invalid = Point2::new(0_usize, 0_usize);

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_i32() {
        let validator = OnlyOneZero;

        let p_valid1 = Point2::new(0_i32, 5_i32);
        let p_valid2 = Point2::new(-3_i32, 0_i32);
        let p_invalid = Point2::new(0_i32, 0_i32);

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_f64() {
        let validator = OnlyOneZero;

        let p_valid1 = Point2::new(0.0, 1.5);
        let p_valid2 = Point2::new(-2.3, 0.0);
        let p_invalid = Point2::new(0.0, 0.0);

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_length() {
        let validator = OnlyOneZero;

        let p_valid1 = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(2.0));
        let p_valid2 = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(0.0));
        let p_invalid = Point2::new(Length::new::<meter>(0.0), Length::new::<meter>(0.0));

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_angle() {
        let validator = OnlyOneZero;

        let p_valid1 = Point2::new(Angle::new::<radian>(0.0), Angle::new::<radian>(1.0));
        let p_valid2 = Point2::new(Angle::new::<radian>(2.0), Angle::new::<radian>(0.0));
        let p_invalid = Point2::new(Angle::new::<radian>(0.0), Angle::new::<radian>(0.0));

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_tuple_usize() {
        let validator = OnlyOneZero;

        let p_valid1 = (0_usize, 5_usize);
        let p_valid2 = (3_usize, 0_usize);
        let p_invalid = (0_usize, 0_usize);

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_tuple_i32() {
        let validator = OnlyOneZero;

        let p_valid1 = (0_i32, 5_i32);
        let p_valid2 = (-3_i32, 0_i32);
        let p_invalid = (0_i32, 0_i32);

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_tuple_f64() {
        let validator = OnlyOneZero;

        let p_valid1 = (0.0, 1.5);
        let p_valid2 = (-2.3, 0.0);
        let p_invalid = (0.0, 0.0);

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_tuple_length() {
        let validator = OnlyOneZero;

        let p_valid1 = (Length::new::<meter>(0.0), Length::new::<meter>(2.0));
        let p_valid2 = (Length::new::<meter>(1.0), Length::new::<meter>(0.0));
        let p_invalid = (Length::new::<meter>(0.0), Length::new::<meter>(0.0));

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }

    #[test]
    fn test_only_one_zero_tuple_angle() {
        let validator = OnlyOneZero;

        let p_valid1 = (Angle::new::<radian>(0.0), Angle::new::<radian>(1.0));
        let p_valid2 = (Angle::new::<radian>(2.0), Angle::new::<radian>(0.0));
        let p_invalid = (Angle::new::<radian>(0.0), Angle::new::<radian>(0.0));

        assert!(validator.validate(&p_valid1).is_ok());
        assert!(validator.validate(&p_valid2).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
    }
}
