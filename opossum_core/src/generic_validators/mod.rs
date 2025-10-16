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
mod positive;

pub use finite::IsFinite;
pub use in_range::IsInRange;
pub use logical_combinations::{AndValidator, OrValidator};
pub use normal::IsNormal;
pub use not_zero::IsNotZero;
pub use only_one_zero::OnlyOneZero;
pub use positive::IsPositive;

/// Trait for types that can validate a value of type `T`.
///
/// A validator checks a value against some condition and returns
/// `OpmResult<()>`, which is `Ok(())` if validation passes or
/// an error if it fails.
use serde::{Deserialize, Serialize};
pub trait Validate<T:Clone> {
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
pub struct Validated<T:Clone, V: Validate<T>> {
    value: T,
    validator: V,
}



impl<T:Clone, V: Validate<T>> Validated<T, V> {
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
    
    // pub fn get_mut(&mut self) -> ValidatedGuard<'_, T, V> {
        //     ValidatedGuard::new(self) 
    // }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Eq)]
pub struct ValidatedVec<T:Clone, V: Validate<T>> {
    values: Vec<T>,
    validator: V,
}

impl<T:Clone, V: Validate<T>> ValidatedVec<T, V> {
    pub fn new(values: Vec<T>, validator: V) -> OpmResult<Self> {
        for v in &values {
            validator.validate(v)?;
        }
        Ok(Self { values, validator })
    }

    pub fn get(&self) -> &Vec<T> {
        &self.values
    }

    pub fn get_mut_at_index(&mut self, index: usize) -> OpmResult<ValidatedItemGuard<'_, T, V>> {
        if let Some(backup) = self.values.get(index).cloned() {
            Ok(ValidatedItemGuard {
                parent: self,
                index,
                backup,
                state: GuardState::Pending,
            })
        } else {
            Err(OpossumError::Other("Index to create ValidatedItemGuard of vector out of bounds!".into()))
        }
    }

    pub fn set(&mut self, new_values: Vec<T>) -> OpmResult<()> {
        for v in &new_values {
            self.validator.validate(v)?;
        }
        self.values = new_values;
        Ok(())
    }
}

enum GuardState {
    Pending,       
    Committed,     
    Failed,        
}

pub struct ValidatedItemGuard<'a, T:Clone, V: Validate<T>> {
    parent: &'a mut ValidatedVec<T, V>,
    index: usize,
    backup: T,
    state: GuardState,
}

impl<'a, T:Clone, V: Validate<T>> Deref for ValidatedItemGuard<'a, T, V> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.parent.values[self.index]
    }
}

impl<'a, T:Clone, V: Validate<T>> DerefMut for ValidatedItemGuard<'a, T, V> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.parent.values[self.index]
    }
}

impl<'a, T:Clone, V: Validate<T>> ValidatedItemGuard<'a, T, V> {
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

impl<'a, T: Clone, V: Validate<T>> Drop for ValidatedItemGuard<'a, T, V> {
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
            }GuardState::Failed => {
                // commit failed → Rollback to Backup
                self.parent.values[self.index] = self.backup.clone();
                log::warn!("Commit failed earlier, rolled back element at index {}", self.index);
            }
            GuardState::Committed => {
                // do nothing
            }
        }
    }
}


#[cfg(test)]
mod tests {
        use crate::utils::test_helper::test_helper::check_logs;
    
        use super::*;
        use log::Level;
    use nalgebra::Point2;

    // Hilfsfunktion, um den Logger einmal pro Test zu initialisieren
    fn setup_logger() {
            static INIT: std::sync::Once = std::sync::Once::new();
            INIT.call_once(|| testing_logger::setup());
        }
    
        #[test]
        fn test_validated_new_and_set_is_positive() {
                let mut v = Validated::new(5, IsPositive).unwrap();
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
        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], IsNotZero).unwrap();
        
        assert!(v.get_mut_at_index(1).is_err());

    }

            #[test]
    fn test_validated_vec_guard_commit_is_err() -> OpmResult<()> {
        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], IsNotZero)?;

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
        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], IsNotZero)?;

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

        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], IsNotZero)?;

        {
            let mut guard = v.get_mut_at_index(0)?;
            guard.x = 0;
            guard.y = 0; // 
            // no commit → Drop calls try_commit() and creates warnings
        }

        check_logs(Level::Warn, vec![
            "Validation failed on drop for index 0: Opossum Error:Other:Value must satisfy |_self, v: &Point2<i32>| !v.x.is_zero() && !v.y.is_zero()",
            "Rolled back element at index 0",
        ]);

        let val = &v.get()[0];
        assert_eq!(val.x, 1);
        assert_eq!(val.y, 2);

        Ok(())
    }

    #[test]
    fn test_validated_vec_guard_drop_successful_logs_is_not_zero() -> OpmResult<()> {
        setup_logger();

        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], IsNotZero)?;

        {
            let mut guard = v.get_mut_at_index(0)?;
            guard.x = 3;
            guard.y = 1; // valid
            // no commit → Drop calls try_commit() and creates info
        }

        check_logs(Level::Info, vec![
            "Forced committing was successful!",
        ]);

        let val = &v.get()[0];
        assert_eq!(val.x, 3);
        assert_eq!(val.y, 1);

        Ok(())
    }

    #[test]
    fn test_validated_vec_multiple_changes_try_validate_and_update_backup() -> OpmResult<()> {
        let mut v = ValidatedVec::new(vec![Point2::new(1, 2)], IsNotZero)?;

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