use std::ops::Range;

use crate::impl_validator;
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Energy, Length};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct AllPositive;

impl_validator!(AllPositive, |_self, v: &i32| *v >= 0, i32);
impl_validator!(AllPositive, |_self, v: &f64| v.is_sign_positive(), f64);
impl_validator!(AllPositive, |_self, v: &Angle| v.is_sign_positive(), Angle);
impl_validator!(
    AllPositive,
    |_self, v: &Energy| v.is_sign_positive(),
    Energy
);
impl_validator!(
    AllPositive,
    |_self, v: &Length| v.is_sign_positive(),
    Length
);
impl_validator!(
    AllPositive,
    |_self, v: &Point2<f64>| v.x.is_sign_positive() && v.y.is_sign_positive(),
    Point2<f64>
);
impl_validator!(
    AllPositive,
    |_self, v: &Point2<Length>| v.x.is_sign_positive() && v.y.is_sign_positive(),
    Point2<Length>
);
impl_validator!(
    AllPositive,
    |_self, v: &(f64, f64)| v.0.is_sign_positive() && v.1.is_sign_positive(),
    (f64, f64)
);
impl_validator!(
    AllPositive,
    |_self, v: &(Length, Length)| v.0.is_sign_positive() && v.1.is_sign_positive(),
    (Length, Length)
);
impl_validator!(
    AllPositive,
    |_self, v: &Range<Length>| v.start.is_sign_positive() && v.end.is_sign_positive(),
    Range<Length>
);

impl_validator!(
    AllPositive,
    |_self, v: &(Length, Energy)| v.0.is_sign_positive() && v.1.is_sign_positive(),
    (Length, Energy)
);

impl_validator!(
    AllPositive,
    |_self, v: &(Length, f64)| v.0.is_sign_positive() && v.1.is_sign_positive(),
    (Length, f64)
);

impl_validator!(
    AllPositive,
    |_self, v: &Vec<Point2<f64>>| v.iter().all(|val| val.x.is_sign_positive() && val.y.is_sign_positive()),
    Vec<Point2<f64>>
);

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct XPositive;

impl_validator!(
    XPositive,
    |_self, v: &Point2<f64>| v.x.is_sign_positive(),
    Point2<f64>
);
impl_validator!(
    XPositive,
    |_self, v: &Point2<Length>| v.x.is_sign_positive(),
    Point2<Length>
);

impl_validator!(
    XPositive,
    |_self, v: &(Length, f64)| v.0.is_sign_positive(),
    (Length, f64)
);

impl_validator!(
    XPositive,
    |_self, v: &(Length, Energy)| v.0.is_sign_positive(),
    (Length, Energy)
);

impl_validator!(
    XPositive,
    |_self, v: &Vec<(Length, Energy)>| v.iter().all(|val| val.0.is_sign_positive()),
    Vec<(Length, Energy)>
);

impl_validator!(
    XPositive,
    |_self, v: &Vec<Point2<Length>>| v.iter().all(|val| val.x.is_sign_positive()),
    Vec<Point2<Length>>
);

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct YPositive;

impl_validator!(
    YPositive,
    |_self, v: &Point2<f64>| v.y.is_sign_positive(),
    Point2<f64>
);
impl_validator!(
    YPositive,
    |_self, v: &Point2<Length>| v.y.is_sign_positive(),
    Point2<Length>
);

impl_validator!(
    YPositive,
    |_self, v: &(Length, Energy)| v.1.is_sign_positive(),
    (Length, Energy)
);

impl_validator!(
    YPositive,
    |_self, v: &(Length, f64)| v.1.is_sign_positive(),
    (Length, f64)
);

impl_validator!(
    YPositive,
    |_self, v: &Vec<(Length, Energy)>| v.iter().all(|val| val.1.is_sign_positive()),
    Vec<(Length, Energy)>
);

impl_validator!(
    YPositive,
    |_self, v: &Vec<Point2<Length>>| v.iter().all(|val| val.y.is_sign_positive()),
    Vec<Point2<Length>>
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
}
