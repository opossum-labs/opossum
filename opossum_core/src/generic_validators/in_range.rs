use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{Target, Validate, ValidateVec, numlike::NumLike},
};
use nalgebra::Point2;
use opm_macros_lib::ValidateNumeric;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Validator that checks if a value is within a specified range.
///
/// `IsInRange` can be inclusive or exclusive of the boundaries.
///
/// # Type Parameters
///
/// * `T` - The type of the value to validate. Must implement `PartialOrd`.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, ValidateNumeric)]
#[rule(
    is_in_range,
    message = "All value must be in range!",
    target = "both",
    mode = "all",
    on = "self"
)]
pub struct AllInRange<T: NumLike> {
    min: T,
    max: T,
    inclusive: bool,
}

impl<T: NumLike> Default for AllInRange<T> {
    fn default() -> Self {
        panic!(
            "AllInRange::default() is a dummy implementation to facilitate using serde(skip) on Validator fields in the Validated struct!\nAlways implement Deserialize a manually for every struct that holds a validated type with an AllInRange Validator to ensure that all parameters are set correctly!"
        );
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, ValidateNumeric)]
#[rule(
    is_in_range,
    message = "X-value must be in range!",
    target = "x",
    mode = "all",
    on = "self"
)]
#[allow(dead_code)]
pub struct XInRange<T: NumLike> {
    min: T,
    max: T,
    inclusive: bool,
}
impl<T: NumLike> Default for XInRange<T> {
    fn default() -> Self {
        panic!(
            "XInRange::default() is a dummy implementation to facilitate using serde(skip) on Validator fields in the Validated struct! Always\nimplement Deserialize a manually for every struct that holds a validated type with an AllInRange Validator to ensure that all parameters are set correctly!"
        );
    }
}

#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq, ValidateNumeric)]
#[rule(
    is_in_range,
    message = "Y-value must be in range!",
    target = "y",
    mode = "all",
    on = "self"
)]
#[allow(dead_code)]
pub struct YInRange<T: NumLike> {
    min: T,
    max: T,
    inclusive: bool,
}
impl<T: NumLike> Default for YInRange<T> {
    fn default() -> Self {
        panic!(
            "YInRange::default() is a dummy implementation to facilitate using serde(skip) on Validator fields in the Validated struct!\nAlways implement Deserialize a manually for every struct that holds a validated type with an AllInRange Validator to ensure that all parameters are set correctly!"
        );
    }
}

impl<T: NumLike> AllInRange<T> {
    /// Create a new `IsInRange` validator.
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum boundary.
    /// * `max` - Maximum boundary.
    /// * `inclusive` - Whether the boundaries are inclusive.
    ///
    /// # Returns
    ///
    /// * `Ok(IsInRange)` if `min < max`.
    ///
    /// # Errors
    ///
    /// Returns `OpossumError::Other` if `min >= max`.
    pub fn new(min: T, max: T, inclusive: bool) -> OpmResult<Self> {
        if min < max {
            Ok(Self {
                min,
                max,
                inclusive,
            })
        } else {
            Err(OpossumError::Other(
                "IsInRange: minimum value must be smaller than maximum value".into(),
            ))
        }
    }

    /// Check if a value is within the range.
    ///
    /// # Arguments
    ///
    /// * `val` - The value to check.
    ///
    /// # Returns
    ///
    /// * `true` if `val` is within the range according to `inclusive`.
    /// * `false` otherwise.
    pub fn is_in_range(&self, val: &T) -> bool {
        if self.inclusive {
            *val >= self.min && *val <= self.max
        } else {
            *val > self.min && *val < self.max
        }
    }
}

impl<T: NumLike> XInRange<T> {
    /// Create a new `IsInRange` validator.
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum boundary.
    /// * `max` - Maximum boundary.
    /// * `inclusive` - Whether the boundaries are inclusive.
    ///
    /// # Returns
    ///
    /// * `Ok(IsInRange)` if `min < max`.
    ///
    /// # Errors
    ///
    /// Returns `OpossumError::Other` if `min >= max`.
    #[allow(dead_code)]
    pub fn new(min: T, max: T, inclusive: bool) -> OpmResult<Self> {
        if min < max {
            Ok(Self {
                min,
                max,
                inclusive,
            })
        } else {
            Err(OpossumError::Other(
                "IsInRange: minimum value must be smaller than maximum value".into(),
            ))
        }
    }

    /// Check if a value is within the range.
    ///
    /// # Arguments
    ///
    /// * `val` - The value to check.
    ///
    /// # Returns
    ///
    /// * `true` if `val` is within the range according to `inclusive`.
    /// * `false` otherwise.
    pub fn is_in_range(&self, val: &T) -> bool {
        if self.inclusive {
            *val >= self.min && *val <= self.max
        } else {
            *val > self.min && *val < self.max
        }
    }
}

impl<T: NumLike> YInRange<T> {
    /// Create a new `IsInRange` validator.
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum boundary.
    /// * `max` - Maximum boundary.
    /// * `inclusive` - Whether the boundaries are inclusive.
    ///
    /// # Returns
    ///
    /// * `Ok(IsInRange)` if `min < max`.
    ///
    /// # Errors
    ///
    /// Returns `OpossumError::Other` if `min >= max`.
    #[allow(dead_code)]
    pub fn new(min: T, max: T, inclusive: bool) -> OpmResult<Self> {
        if min < max {
            Ok(Self {
                min,
                max,
                inclusive,
            })
        } else {
            Err(OpossumError::Other(
                "IsInRange: minimum value must be smaller than maximum value".into(),
            ))
        }
    }

    /// Check if a value is within the range.
    ///
    /// # Arguments
    ///
    /// * `val` - The value to check.
    ///
    /// # Returns
    ///
    /// * `true` if `val` is within the range according to `inclusive`.
    /// * `false` otherwise.
    pub fn is_in_range(&self, val: &T) -> bool {
        if self.inclusive {
            *val >= self.min && *val <= self.max
        } else {
            *val > self.min && *val < self.max
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::Validate;
    use crate::meter;
    use nalgebra::Point2;
    use uom::si::{angle::radian, f64::Angle};

    fn make_all_in_range() -> AllInRange<f64> {
        AllInRange {
            min: 1.0,
            max: 5.0,
            inclusive: true,
        }
    }

    fn make_x_in_range() -> XInRange<f64> {
        XInRange {
            min: 1.0,
            max: 5.0,
            inclusive: true,
        }
    }

    fn make_y_in_range() -> YInRange<f64> {
        YInRange {
            min: 1.0,
            max: 5.0,
            inclusive: true,
        }
    }

    #[test]
    fn test_all_in_range_single_values() {
        let validator = make_all_in_range();

        assert!(validator.validate(&3.0).is_ok());
        assert!(validator.validate(&1.0).is_ok()); // inclusive
        assert!(validator.validate(&5.0).is_ok());

        let validator_exclusive = AllInRange {
            min: 1.0,
            max: 5.0,
            inclusive: false,
        };
        assert!(validator_exclusive.validate(&1.0).is_err());
        assert!(validator_exclusive.validate(&5.0).is_err());
        assert!(validator_exclusive.validate(&3.0).is_ok());
    }

    #[test]
    fn test_x_in_range_point2() {
        let validator = make_x_in_range();
        let p = Point2::new(3.0, 10.0);

        assert!(validator.validate(&p).is_ok());
        let p_out = Point2::new(0.5, 10.0);
        assert!(validator.validate(&p_out).is_err());
    }

    #[test]
    fn test_y_in_range_point2() {
        let validator = make_y_in_range();
        let p = Point2::new(0.0, 4.0);

        assert!(validator.validate(&p).is_ok());
        let p_out = Point2::new(0.0, 5.5);
        assert!(validator.validate(&p_out).is_err());
    }

    #[test]
    fn test_all_in_range_range_struct() {
        let validator = make_all_in_range();
        let r = Range {
            start: 2.0,
            end: 4.0,
        };
        assert!(validator.validate(&r).is_ok());

        let r_out = Range {
            start: 0.0,
            end: 3.0,
        };
        assert!(validator.validate(&r_out).is_err());
    }

    #[test]
    fn test_all_in_range_vec_of_values() {
        let validator = make_all_in_range();
        let vec_ok = vec![2.0, 3.0, 4.0];
        let vec_fail = vec![0.0, 3.0, 4.0];

        assert!(validator.validate_vec(&vec_ok).is_ok());
        assert!(validator.validate_vec(&vec_fail).is_err());
    }

    #[test]
    fn test_all_in_range_vec_of_point2() {
        let validator = make_all_in_range();
        let points_ok = vec![Point2::new(2.0, 3.0), Point2::new(4.0, 5.0)];
        let points_fail = vec![Point2::new(0.5, 3.0), Point2::new(4.0, 5.0)];

        assert!(validator.validate_vec(&points_ok).is_ok());
        assert!(validator.validate_vec(&points_fail).is_err());
    }

    #[test]
    fn test_x_in_range_vec_of_point2() {
        let validator = make_x_in_range();
        let points_ok = vec![Point2::new(2.0, 10.0), Point2::new(4.0, -5.0)];
        let points_fail = vec![Point2::new(0.0, 3.0), Point2::new(4.0, 5.0)];

        assert!(validator.validate_vec(&points_ok).is_ok());
        assert!(validator.validate_vec(&points_fail).is_err());
    }

    #[test]
    fn test_y_in_range_vec_of_point2() {
        let validator = make_y_in_range();
        let points_ok = vec![Point2::new(2.0, 2.0), Point2::new(4.0, 4.5)];
        let points_fail = vec![Point2::new(0.0, 0.0), Point2::new(4.0, 5.5)];

        assert!(validator.validate_vec(&points_ok).is_ok());
        assert!(validator.validate_vec(&points_fail).is_err());
    }

    #[test]
    fn test_is_in_range_f64_inclusive() -> OpmResult<()> {
        let validator = AllInRange::new(1.0, 5.0, true)?;

        assert!(validator.validate(&1.0).is_ok());
        assert!(validator.validate(&5.0).is_ok());
        assert!(validator.validate(&3.0).is_ok());
        assert!(validator.validate(&0.0).is_err());
        assert!(validator.validate(&6.0).is_err());
        Ok(())
    }

    #[test]
    fn test_is_in_range_f64_exclusive() -> OpmResult<()> {
        let validator = AllInRange::new(1.0, 5.0, false)?;

        assert!(validator.validate(&1.0).is_err());
        assert!(validator.validate(&5.0).is_err());
        assert!(validator.validate(&3.0).is_ok());
        Ok(())
    }

    #[test]
    fn test_is_in_range_length() -> OpmResult<()> {
        let validator = AllInRange::new(meter!(1.0), meter!(5.0), true)?;

        assert!(validator.validate(&meter!(1.0)).is_ok());
        assert!(validator.validate(&meter!(5.0)).is_ok());
        assert!(validator.validate(&meter!(0.5)).is_err());
        Ok(())
    }

    #[test]
    fn test_is_in_range_angle() -> OpmResult<()> {
        let validator =
            AllInRange::new(Angle::new::<radian>(0.0), Angle::new::<radian>(3.14), true)?;

        assert!(validator.validate(&Angle::new::<radian>(0.0)).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(3.14)).is_ok());
        assert!(validator.validate(&Angle::new::<radian>(-1.0)).is_err());
        Ok(())
    }

    #[test]
    fn test_is_in_range_point2_f64() -> OpmResult<()> {
        let validator = AllInRange::new(1.0, 5.0, true)?;
        let p_valid = Point2::new(2.0, 3.0);
        let p_invalid = Point2::new(0.0, 4.0);

        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
        Ok(())
    }

    #[test]
    fn test_is_in_range_point2_length() -> OpmResult<()> {
        let validator = AllInRange::new(meter!(1.0), meter!(5.0), true)?;
        let p_valid = meter!(2.0, 3.0);
        let p_invalid = meter!(0.5, 4.0);
        assert!(validator.validate(&p_valid).is_ok());
        assert!(validator.validate(&p_invalid).is_err());
        Ok(())
    }

    #[test]
    #[should_panic]
    fn all_in_range_default_panic() {
        AllInRange::<f64>::default();
    }
    #[test]
    #[should_panic]
    fn x_in_range_default_panic() {
        XInRange::<f64>::default();
    }
    #[test]
    #[should_panic]
    fn y_in_range_default_panic() {
        YInRange::<f64>::default();
    }
}
