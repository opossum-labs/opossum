use std::ops::Range;

use crate::impl_validator;
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct SecondLarger;

impl_validator!(SecondLarger, |_self, v: &(f64, f64)| v.0 < v.1, (f64, f64));
impl_validator!(
    SecondLarger,
    |_self, v: &(Length, Length)| v.0 < v.1,
    (Length, Length)
);

impl_validator!(
    SecondLarger,
    |_self, v: &Point2<f64>| v.x < v.y,
    Point2<f64>
);
impl_validator!(
    SecondLarger,
    |_self, v: &Point2<Length>| v.x < v.y,
    Point2<Length>
);

impl_validator!(
    SecondLarger,
    |_self, v: &Range<Length>| v.start < v.end,
    Range<Length>
);

#[cfg(test)]
mod tests {
    use crate::generic_validators::Validate;

    use super::*;
    use nalgebra::Point2;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    #[test]
    fn test_tuple_f64_valid() {
        let validator = SecondLarger;
        let value = (1.0_f64, 2.0_f64);
        assert!(validator.validate(&value).is_ok());
    }

    #[test]
    fn test_tuple_f64_invalid_equal() {
        let validator = SecondLarger;
        let value = (2.0_f64, 2.0_f64);
        assert!(validator.validate(&value).is_err());
    }

    #[test]
    fn test_tuple_f64_invalid_smaller() {
        let validator = SecondLarger;
        let value = (3.0_f64, 1.0_f64);
        assert!(validator.validate(&value).is_err());
    }

    #[test]
    fn test_tuple_length_valid() {
        let validator = SecondLarger;
        let value = (Length::new::<meter>(1.0), Length::new::<meter>(2.0));
        assert!(validator.validate(&value).is_ok());
    }

    #[test]
    fn test_tuple_length_invalid() {
        let validator = SecondLarger;
        let value = (Length::new::<meter>(2.0), Length::new::<meter>(1.0));
        assert!(validator.validate(&value).is_err());
    }

    #[test]
    fn test_point2_f64_valid() {
        let validator = SecondLarger;
        let p = Point2::new(1.0_f64, 3.0_f64);
        assert!(validator.validate(&p).is_ok());
    }

    #[test]
    fn test_point2_f64_invalid() {
        let validator = SecondLarger;
        let p = Point2::new(5.0_f64, 2.0_f64);
        assert!(validator.validate(&p).is_err());
    }

    #[test]
    fn test_point2_length_valid() {
        let validator = SecondLarger;
        let p = Point2::new(Length::new::<meter>(0.5), Length::new::<meter>(1.5));
        assert!(validator.validate(&p).is_ok());
    }

    #[test]
    fn test_point2_length_invalid_equal() {
        let validator = SecondLarger;
        let p = Point2::new(Length::new::<meter>(1.0), Length::new::<meter>(1.0));
        assert!(validator.validate(&p).is_err());
    }
}
