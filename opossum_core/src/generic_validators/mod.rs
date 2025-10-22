use std::ops::{Deref, DerefMut};

use crate::error::{OpmResult, OpossumError};

mod finite;
mod impl_macro;
mod in_range;
mod logical_combinations;
mod normal;
mod not_empty;
mod not_nan;
mod not_zero;
mod only_one_zero;
mod path_valid;
mod positive;
mod second_larger;

pub use finite::AllFinite;
pub use in_range::AllInRange;
pub use logical_combinations::{AndValidator, NotValidator, OrValidator};
pub use normal::AllNormal;
pub use not_empty::AllNotEmpty;
pub use not_zero::AllNotZero;
pub use only_one_zero::OnlyOneZero;
pub use path_valid::PathValid;
pub use positive::AllPositive;
pub use second_larger::SecondLarger;

/// Trait for types that can validate a value of type `T`.
///
/// A validator checks a value against some condition and returns
/// `OpmResult<()>`, which is `Ok(())` if validation passes or
/// an error if it fails.
use serde::{Deserialize, Serialize};
pub trait Validate<T> {
    /// Validate the given `value`.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to be validated.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if validation succeeds.
    /// # Errors
    /// Returns `Err(OpossumError)` if validation fails.
    fn validate(&self, value: &T) -> OpmResult<()>;
}

/// A wrapper around a value of type `T` that enforces validation
/// using a `Validate<T>` implementor.
///
/// `Validated` ensures that the value is always valid according
/// to the validator.
#[derive(Copy, Clone, PartialEq, Serialize, Deserialize, Debug, Eq)]
pub struct Validated<T, V: Validate<T>> {
    value: T,
    validator: V,
}

impl<T, V: Validate<T>> Validated<T, V> {
    /// Creates a new `Validated` value.
    ///
    /// # Arguments
    ///
    /// * `value` - The initial value to store.
    /// * `validator` - The validator used to enforce rules.
    ///
    /// # Returns
    ///
    /// * `Ok(Validated)` if the value passes validation.
    ///
    /// # Errors
    /// * Returns `Err(OpossumError)` if the value fails validation.
    pub fn new(value: T, validator: V) -> OpmResult<Self> {
        validator.validate(&value)?;
        Ok(Self { value, validator })
    }

    /// Get a reference to the underlying value.
    pub const fn get(&self) -> &T {
        &self.value
    }

    /// Set a new value, validating it before assignment.
    ///
    /// # Arguments
    ///
    /// * `new_value` - The new value to assign.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the new value passes validation.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if the new value fails validation.
    pub fn set(&mut self, new_value: T) -> OpmResult<()> {
        self.validator.validate(&new_value)?;
        self.value = new_value;
        Ok(())
    }

    /// Consume the `Validated` wrapper and return the inner value.
    ///
    /// This does not perform validation since the value is already guaranteed
    /// to be valid.
    pub fn into_inner(self) -> T {
        self.value
    }
}

/// A wrapper around a value of type `Vec<T>` that enforces validation for all elements of Vec
/// using a `ValidateVec<T>` implementor.
///
/// `ValidatedVec` ensures that the values are always valid according
/// to the validator which is the same for all values.
#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Eq)]
pub struct ValidatedVec<T: Clone, V: Validate<T>> {
    values: Vec<T>,
    validator: V,
}

impl<T: Clone, V: Validate<T>> ValidatedVec<T, V> {
    /// Creates a new `ValidatedVec` by validating all initial values.
    ///
    /// # Arguments
    ///
    /// * `values` - A vector of initial values to store.
    /// * `validator` - The validator used to enforce rules for each element.
    ///
    /// # Returns
    ///
    /// * `Ok(ValidatedVec)` if all values pass validation.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if any value fails validation.
    pub fn new(values: Vec<T>, validator: V) -> OpmResult<Self> {
        for v in &values {
            validator.validate(v)?;
        }
        Ok(Self { values, validator })
    }

    /// Returns a reference to the internal vector.
    ///
    /// # Returns
    ///
    /// * `&Vec<T>` - Reference to the stored values.
    pub const fn get(&self) -> &Vec<T> {
        &self.values
    }

    /// Returns a mutable guard for the element at the given index.
    ///
    /// The guard allows modifying the element while ensuring validation and
    /// automatic rollback on drop if commit is not successful.
    ///
    /// # Arguments
    ///
    /// * `index` - Index of the element to obtain a guard for.
    ///
    /// # Returns
    ///
    /// * `Ok(ValidatedItemGuard)` if the index is valid.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if the index is out of bounds.
    pub fn get_mut_at_index(&mut self, index: usize) -> OpmResult<ValidatedItemGuard<'_, T, V>> {
        self.values.get(index).cloned().map_or_else(
            || {
                Err(OpossumError::Other(
                    "Index to create ValidatedItemGuard of vector out of bounds!".into(),
                ))
            },
            |backup| {
                Ok(ValidatedItemGuard {
                    parent: self,
                    index,
                    backup,
                    state: GuardState::Pending,
                })
            },
        )
    }

    /// Replaces all values in the vector with new values after validation.
    ///
    /// # Arguments
    ///
    /// * `new_values` - A vector of new values to store.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if all new values pass validation.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if any new value fails validation.
    pub fn set(&mut self, new_values: Vec<T>) -> OpmResult<()> {
        for v in &new_values {
            self.validator.validate(v)?;
        }
        self.values = new_values;
        Ok(())
    }
}

/// Represents the validation state of a `ValidatedItemGuard`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuardState {
    /// Guard has been created but not yet committed.
    Pending,
    /// Value has been successfully committed.
    Committed,
    /// Commit has failed at least once.
    Failed,
}

/// A guard for a single element in a `ValidatedVec`.
///
/// Allows mutable access to an element while enforcing validation rules,
/// and performs automatic rollback if commit is not successful.
pub struct ValidatedItemGuard<'a, T: Clone, V: Validate<T>> {
    parent: &'a mut ValidatedVec<T, V>,
    index: usize,
    backup: T,
    state: GuardState,
}

impl<T: Clone, V: Validate<T>> Deref for ValidatedItemGuard<'_, T, V> {
    type Target = T;
    /// Returns an immutable reference to the guarded element.
    ///
    /// # Returns
    ///
    /// * `&T` - Reference to the element.
    fn deref(&self) -> &Self::Target {
        &self.parent.values[self.index]
    }
}

impl<T: Clone, V: Validate<T>> DerefMut for ValidatedItemGuard<'_, T, V> {
    /// Returns a mutable reference to the guarded element.
    ///
    /// # Returns
    ///
    /// * `&mut T` - Mutable reference to the element.
    fn deref_mut(&mut self) -> &mut T {
        &mut self.parent.values[self.index]
    }
}

impl<T: Clone, V: Validate<T>> ValidatedItemGuard<'_, T, V> {
    /// Commits the current value, marking it as validated and final.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the value passes validation.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if the value fails validation.
    pub fn commit(mut self) -> OpmResult<()> {
        let val = &self.parent.values[self.index];
        match self.parent.validator.validate(val) {
            Ok(()) => {
                self.state = GuardState::Committed;
                Ok(())
            }
            Err(e) => {
                self.state = GuardState::Failed;
                Err(e)
            }
        }
    }

    /// Validates the current value and updates the backup for rollback.
    ///
    /// This method does not mark the value as fully committed. It is useful
    /// for previewing changes and ensuring the backup reflects the latest
    /// valid state.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the value passes validation and backup is updated.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if the value fails validation.
    fn validate_and_update_backup(&mut self) -> OpmResult<()> {
        let val = &mut self.parent.values[self.index];
        match self.parent.validator.validate(val) {
            Ok(()) => {
                self.backup = val.clone();
                self.state = GuardState::Pending;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

impl<T: Clone, V: Validate<T>> Drop for ValidatedItemGuard<'_, T, V> {
    /// Drop handler that enforces validation and performs rollback if necessary.
    ///
    /// If the guard is still pending and validation fails, the value is rolled
    /// back to the last valid backup. If the guard previously failed commit,
    /// the value is also rolled back. Committed values are not modified.
    fn drop(&mut self) {
        match self.state {
            GuardState::Pending => {
                if let Err(e) = self.validate_and_update_backup() {
                    log::warn!("Validation failed on drop for index {}: {}", self.index, e);
                    self.parent.values[self.index] = self.backup.clone();
                    log::warn!("Rolled back element at index {}", self.index);
                } else {
                    log::info!("Forced committing was successful!");
                }
            }
            GuardState::Failed => {
                // commit failed → Rollback to Backup
                self.parent.values[self.index] = self.backup.clone();
                log::warn!(
                    "Commit failed earlier, rolled back element at index {}",
                    self.index
                );
            }
            GuardState::Committed => {
                // do nothing
            }
        }
    }
}

//helper trait for EnsureValidate Macro
pub trait ValidateTrait {}

impl<T, V: Validate<T>> ValidateTrait for Validated<T, V> {}
impl<T: Clone, V: Validate<T>> ValidateTrait for ValidatedVec<T, V> {}

#[cfg(test)]
mod tests {
    use crate::utils::test_helper::test_helper::check_logs;

    use super::*;
    use log::Level;
    use nalgebra::Point2;

    fn setup_logger() {
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| testing_logger::setup());
    }

    #[test]
    fn test_validated_new_and_set_is_positive() {
        let mut v = Validated::new(5, AllPositive).unwrap();
        assert_eq!(*v.get(), 5);

        // Set valid value
        assert!(v.set(10).is_ok());
        assert_eq!(*v.get(), 10);

        // Set invalid value
        assert!(v.set(-2).is_err());
        assert_eq!(*v.get(), 10); // values remains 10
    }

    #[test]
    fn test_validated_vec_guard_invalid_index() {
        let mut v: ValidatedVec<nalgebra::OPoint<i32, nalgebra::Const<2>>, AllNotZero> =
            ValidatedVec::new(vec![Point2::new(1, 2)], AllNotZero).unwrap();

        assert!(v.get_mut_at_index(1).is_err());
    }

    #[test]
    fn test_validated_vec_guard_commit_is_err() -> OpmResult<()> {
        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], AllNotZero)?;

        {
            let mut guard = v.get_mut_at_index(0)?;
            guard.x = 0;
            guard.y = 1;
            assert!(guard.commit().is_err());
        }

        let val = &v.get()[0];
        assert_eq!(val.x, 1);
        assert_eq!(val.y, 2);

        Ok(())
    }

    #[test]
    fn test_validated_vec_guard_commit_is_ok() -> OpmResult<()> {
        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], AllNotZero)?;

        {
            let mut guard = v.get_mut_at_index(0)?;
            guard.x = 3;
            guard.y = 1;
            assert!(guard.commit().is_ok());
        }

        let val = &v.get()[0];
        assert_eq!(val.x, 3);
        assert_eq!(val.y, 1);

        Ok(())
    }

    #[test]
    fn test_validated_vec_guard_drop_logs_is_not_zero() -> OpmResult<()> {
        setup_logger();

        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], AllNotZero)?;

        {
            let mut guard = v.get_mut_at_index(0)?;
            guard.x = 0;
            guard.y = 0; // 
            // no commit → Drop calls try_commit() and creates warnings
        }

        check_logs(
            Level::Warn,
            vec![
                "Validation failed on drop for index 0: Opossum Error:Other:Value must satisfy |_self, v: &Point2<i32>| !v.x.is_zero() && !v.y.is_zero()",
                "Rolled back element at index 0",
            ],
        );

        let val = &v.get()[0];
        assert_eq!(val.x, 1);
        assert_eq!(val.y, 2);

        Ok(())
    }

    #[test]
    fn test_validated_vec_guard_drop_successful_logs_is_not_zero() -> OpmResult<()> {
        setup_logger();

        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], AllNotZero)?;

        {
            let mut guard = v.get_mut_at_index(0)?;
            guard.x = 3;
            guard.y = 1; // valid
            // no commit → Drop calls try_commit() and creates info
        }

        check_logs(Level::Info, vec!["Forced committing was successful!"]);

        let val = &v.get()[0];
        assert_eq!(val.x, 3);
        assert_eq!(val.y, 1);

        Ok(())
    }

    #[test]
    fn test_validated_vec_multiple_changes_try_validate_and_update_backup() -> OpmResult<()> {
        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], AllNotZero)?;

        {
            let mut guard = v.get_mut_at_index(0)?;
            guard.x = 5;
            guard.y = 3;
            assert!(guard.validate_and_update_backup().is_ok());

            guard.x = 0;
            guard.y = 0;
            assert!(guard.validate_and_update_backup().is_err());
        }

        let val = &v.get()[0];
        assert_eq!(val.x, 5);
        assert_eq!(val.y, 3);

        Ok(())
    }
}
