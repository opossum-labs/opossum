use std::ops::Range;

use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{NumLike, Validate},
};
use nalgebra::Point2;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct SecondLarger;

impl<T: NumLike> Validate<(T, T)> for SecondLarger {
    fn validate(&self, val: &(T, T)) -> OpmResult<()> {
        if val.0.smaller_than(&val.1) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Second value must be larger than first!".into(),
            ))
        }
    }
}
impl<T: NumLike + 'static> Validate<Point2<T>> for SecondLarger {
    fn validate(&self, val: &Point2<T>) -> OpmResult<()> {
        if val.x.smaller_than(&val.y) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Second value must be larger than first!".into(),
            ))
        }
    }
}

impl<T: NumLike> Validate<Range<T>> for SecondLarger {
    fn validate(&self, val: &Range<T>) -> OpmResult<()> {
        if val.start.smaller_than(&val.end) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Second value must be larger than first!".into(),
            ))
        }
    }
}

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
