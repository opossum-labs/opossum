use crate::error::OpmResult;

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
