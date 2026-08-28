use std::marker::PhantomData;
use std::ops::Range;

use nalgebra::Point2;
use serde::{Deserialize, Serialize};

use crate::error::{OpmResult, OpossumError};
use crate::generic_validators::{NumLike, Validate, ValidateVec};

/// Trait to define static bounds at compile time.
pub trait StaticBounds<T: NumLike> {
    fn min() -> T;
    fn max() -> T;
    fn inclusive() -> bool;
}

/// A zero-sized validator that checks bounds statically.
// ❌ Removed `ValidateNumeric` and `#[rule(...)]` here.
#[derive(Copy, Clone, PartialEq, Debug, Serialize, Deserialize, Eq)]
pub struct StaticInRange<T: NumLike, B: StaticBounds<T>> {
    _marker: PhantomData<(T, B)>,
}

// Implement a trivial, non-panicking Default.
// This makes Serde happy when using #[serde(skip)] on the validator.
impl<T: NumLike, B: StaticBounds<T>> Default for StaticInRange<T, B> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T: NumLike, B: StaticBounds<T>> StaticInRange<T, B> {
    /// The core validation logic.
    pub fn is_in_range(&self, val: &T) -> bool {
        let min = B::min();
        let max = B::max();
        if B::inclusive() {
            *val >= min && *val <= max
        } else {
            *val > min && *val < max
        }
    }
}

// --- Manual Trait Implementations (replacing the macro) ---
// These explicitly support both generic parameters <T, B>.

impl<T: NumLike, B: StaticBounds<T>> Validate<T> for StaticInRange<T, B> {
    fn validate(&self, val: &T) -> OpmResult<()> {
        if self.is_in_range(val) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Value is outside of static bounds!".into(),
            ))
        }
    }
}

impl<T: NumLike, B: StaticBounds<T>> Validate<(T, T)> for StaticInRange<T, B> {
    fn validate(&self, val: &(T, T)) -> OpmResult<()> {
        if self.is_in_range(&val.0) && self.is_in_range(&val.1) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Value is outside of static bounds!".into(),
            ))
        }
    }
}

impl<T: NumLike + 'static, B: StaticBounds<T>> Validate<Point2<T>> for StaticInRange<T, B> {
    fn validate(&self, val: &Point2<T>) -> OpmResult<()> {
        if self.is_in_range(&val.x) && self.is_in_range(&val.y) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Value is outside of static bounds!".into(),
            ))
        }
    }
}

impl<T: NumLike + 'static, B: StaticBounds<T>> Validate<Range<T>> for StaticInRange<T, B> {
    fn validate(&self, val: &Range<T>) -> OpmResult<()> {
        if self.is_in_range(&val.start) && self.is_in_range(&val.end) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Value is outside of static bounds!".into(),
            ))
        }
    }
}

impl<T: NumLike, B: StaticBounds<T>> ValidateVec<T> for StaticInRange<T, B> {
    fn validate_vec(&self, val: &[T]) -> OpmResult<()> {
        if val.iter().all(|v| self.is_in_range(v)) {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Value is outside of static bounds!".into(),
            ))
        }
    }
}

impl<T: NumLike, B: StaticBounds<T>> ValidateVec<(T, T)> for StaticInRange<T, B> {
    fn validate_vec(&self, val: &[(T, T)]) -> OpmResult<()> {
        if val
            .iter()
            .all(|v| self.is_in_range(&v.0) && self.is_in_range(&v.1))
        {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Value is outside of static bounds!".into(),
            ))
        }
    }
}

impl<T: NumLike + 'static, B: StaticBounds<T>> ValidateVec<Point2<T>> for StaticInRange<T, B> {
    fn validate_vec(&self, val: &[Point2<T>]) -> OpmResult<()> {
        if val
            .iter()
            .all(|v| self.is_in_range(&v.x) && self.is_in_range(&v.y))
        {
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Value is outside of static bounds!".into(),
            ))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::{Validate, Validated};
    use nalgebra::Point2;
    // We assume `ron` is available for testing serialization,
    // as it is used in other OPOSSUM tests (e.g., refr_index_const.rs).
    use ron;

    // --- 1. Define Mock Boundaries for Testing ---

    /// A mock boundary for testing purposes, allowing values from 0.0 to 10.0 (inclusive).
    #[derive(Copy, Clone, PartialEq, Debug, Eq)]
    struct TestBounds;

    impl StaticBounds<f64> for TestBounds {
        fn min() -> f64 {
            0.0
        }
        fn max() -> f64 {
            10.0
        }
        fn inclusive() -> bool {
            true
        }
    }

    /// Alias for our test validator to keep signatures clean.
    type TestValidator = StaticInRange<f64, TestBounds>;

    // --- 2. Test Basic Validation ---

    #[test]
    fn test_valid_scalar() {
        let validator = TestValidator::default();
        let val = Validated::new(5.0, validator);

        // Assert that creation succeeded and holds the correct value.
        assert!(val.is_ok());
        assert_eq!(*val.unwrap().get(), 5.0);
    }

    #[test]
    fn test_invalid_scalar_too_small() {
        let validator = TestValidator::default();
        let val = Validated::new(-1.0, validator);

        // Assert that values below the minimum are rejected.
        assert!(val.is_err());
    }

    #[test]
    fn test_invalid_scalar_too_large() {
        let validator = TestValidator::default();
        let val = Validated::new(11.0, validator);

        // Assert that values above the maximum are rejected.
        assert!(val.is_err());
    }

    #[test]
    fn test_boundaries_inclusive() {
        let validator = TestValidator::default();

        // Since inclusive() is true, exactly 0.0 and 10.0 must be accepted.
        assert!(Validated::new(0.0, validator).is_ok());
        assert!(Validated::new(10.0, validator).is_ok());
    }

    // --- 3. Test Complex Data Structures ---

    #[test]
    fn test_point2_validation() {
        let validator = TestValidator::default();

        let valid_point = Point2::new(2.0, 8.0);
        assert!(validator.validate(&valid_point).is_ok());

        // Invalid X coordinate.
        let invalid_point_x = Point2::new(11.0, 5.0);
        assert!(validator.validate(&invalid_point_x).is_err());

        // Invalid Y coordinate.
        let invalid_point_y = Point2::new(5.0, -2.0);
        assert!(validator.validate(&invalid_point_y).is_err());
    }

    #[test]
    fn test_tuple_validation() {
        let validator = TestValidator::default();

        let valid_tuple = (4.0, 6.0);
        assert!(validator.validate(&valid_tuple).is_ok());

        let invalid_tuple = (15.0, 6.0);
        assert!(validator.validate(&invalid_tuple).is_err());
    }

    // --- 4. Test Serialization & Deserialization ---

    #[test]
    fn test_serialization_deserialization() {
        let validator = TestValidator::default();
        let val =
            Validated::new(5.0, validator).expect("Failed to create valid value for Serde test");

        // Serialize the validated struct.
        let serialized = ron::ser::to_string(&val).expect("Failed to serialize Validated struct");

        // The serialized output should ideally only contain the value,
        // since the validator is a Zero-Sized Type and often skipped or transparent.
        // Let's deserialize it back into a new object.
        let deserialized: Validated<f64, TestValidator> =
            ron::de::from_str(&serialized).expect("Failed to deserialize into Validated struct");

        // Ensure the value was preserved.
        assert_eq!(*deserialized.get(), 5.0);

        // Crucial test: Ensure the validator logic survived the deserialization.
        // If we change the value to something out of bounds, it must fail.
        let mut mutable_val = deserialized;

        let update_err = mutable_val.set(15.0);
        assert!(
            update_err.is_err(),
            "Deserialized validator failed to reject out-of-bounds value"
        );

        let update_ok = mutable_val.set(7.0);
        assert!(
            update_ok.is_ok(),
            "Deserialized validator rejected a valid in-bounds value"
        );
        assert_eq!(*mutable_val.get(), 7.0);
    }
    #[test]
    fn test_deserialization_rejects_invalid_ron() {
        // Provide a RON string with a value clearly outside the 0.0 - 10.0 bounds
        let invalid_ron = "15.0";

        // Attempt to deserialize directly into our Validated type
        let deserialized: Result<Validated<f64, TestValidator>, _> = ron::de::from_str(invalid_ron);

        // Assert that the deserialization fails due to the validation logic
        assert!(
            deserialized.is_err(),
            "Deserialization succeeded with an invalid value, bypassing validation!"
        );
    }
}
