use crate::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, ops::Index};
use utoipa::openapi::{RefOr, Schema};

mod finite;
mod in_range;
mod logical_combinations;
mod macros;
mod min_entries;
mod normal;
mod not_all_zero;
mod not_empty;
mod not_nan;
mod not_zero;
mod numlike;
mod only_one_zero;
mod pass;
mod path_valid;
pub mod positive;
mod second_larger;
mod static_in_range;

pub use finite::{AllFinite, XFinite, YFinite};
pub use in_range::AllInRange;
pub use logical_combinations::{
    AndValidator, AndValidatorVec, NotValidator, NotValidatorVec, OrValidator, OrValidatorVec,
};
pub use min_entries::Min3Entries;
pub use normal::{AllNormal, XNormal, YNormal};
pub use not_all_zero::{NotAllZero, XNotAllZero, YNotAllZero};
pub use not_empty::AllNotEmpty;
pub use not_nan::{AllNotNan, XNotNan, YNotNan};
pub use not_zero::AllNotZero;
pub use numlike::NumLike;
pub use only_one_zero::OnlyOneZero;
pub use pass::Pass;
pub use path_valid::PathValid;
pub use positive::{AllPositive, XPositive, YPositive};
pub use second_larger::SecondLarger;
pub use static_in_range::{StaticBounds, StaticInRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    X,
    Y,
    Both,
}

/// Trait for types that can validate a value of type `T`.
///
/// A validator checks a value against some condition and returns
/// `OpmResult<()>`, which is `Ok(())` if validation passes or
/// an error if it fails.
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

/// Trait for types that can validate a Vector of type  `T` as a whole.
///
/// A validator checks the vector against some condition and returns
/// `OpmResult<()>`, which is `Ok(())` if validation passes or
/// an error if it fails.
pub trait ValidateVec<T> {
    /// Validate a vector
    ///
    /// # Errors
    /// Returns an error if validation fails
    fn validate_vec(&self, values: &[T]) -> OpmResult<()>;
}

/// A wrapper around a value of type `T` that enforces validation
/// using a `Validate<T>` implementor.
///
/// `Validated` ensures that the value is always valid according
/// to the validator.
#[derive(Copy, Clone, PartialEq, Serialize, Debug, Eq)]
#[serde(transparent)]
pub struct Validated<T, V: Validate<T>> {
    value: T,
    #[serde(skip)]
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

impl<'de, T, V> Deserialize<'de> for Validated<T, V>
where
    T: Deserialize<'de>,
    V: Validate<T> + Default,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize the inner value directly (matching #[serde(transparent)])
        let value = T::deserialize(deserializer)?;

        // Instantiate the zero-sized validator
        let validator = V::default();

        // Run the validation logic. If it fails, map the OpossumError to a serde error.
        Self::new(value, validator).map_err(serde::de::Error::custom)
    }
}

impl<T, V: Validate<T>> utoipa::ToSchema for Validated<T, V>
where
    T: utoipa::ToSchema,
{
    fn name() -> Cow<'static, str> {
        // delegiere einfach den Schema-Namen an T
        T::name()
    }
}

impl<T, V: Validate<T>> utoipa::PartialSchema for Validated<T, V>
where
    T: utoipa::PartialSchema,
{
    fn schema() -> RefOr<Schema> {
        // delegiere das tatsächliche Schema an T
        T::schema()
    }
}

/// A wrapper around a value of type `Vec<T>` that enforces validation for all elements of Vec
/// using a `ValidateVec<T>` implementor.
///
/// `ValidatedVec` ensures that the values are always valid according
/// to the validator which is the same for all values.
#[derive(Clone, PartialEq, Serialize, Debug, Eq)]
pub struct ValidatedVec<T: Clone, EV: Validate<T>, CV: ValidateVec<T>> {
    values: Vec<T>,
    #[serde(skip)]
    element_validator: EV,
    #[serde(skip)]
    container_validator: CV,
}
// Private helper struct to match the default serialization format: {"values": [...]}
#[derive(Deserialize)]
struct ValidatedVecHelper<T> {
    values: Vec<T>,
}

// Manually implement Deserialize to guarantee validation upon loading
impl<'de, T, EV, CV> Deserialize<'de> for ValidatedVec<T, EV, CV>
where
    T: Clone + Deserialize<'de>,
    EV: Validate<T> + Default,
    CV: ValidateVec<T> + Default,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize into the helper to extract the vector of values
        let helper = ValidatedVecHelper::<T>::deserialize(deserializer)?;

        // Instantiate the zero-sized validators
        let element_validator = EV::default();
        let container_validator = CV::default();

        // Validate elements and the container. Return a serde error if validation fails.
        Self::new(helper.values, element_validator, container_validator)
            .map_err(serde::de::Error::custom)
    }
}

impl<T: Clone, EV: Validate<T>, CV: ValidateVec<T>> ValidatedVec<T, EV, CV> {
    /// Creates a new `ValidatedVec` from an initial vector of values.
    ///
    /// This validates each element using `element_validator` and the entire
    /// vector using `container_validator`.
    ///
    /// # Arguments
    ///
    /// * `values` - Initial vector of values to store.
    /// * `element_validator` - Validator applied to each element individually.
    /// * `container_validator` - Validator applied to the vector as a whole.
    ///
    /// # Returns
    ///
    /// * `Ok(ValidatedVec)` if all elements and the container pass validation.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if any element or the container fails validation.
    pub fn new(values: Vec<T>, element_validator: EV, container_validator: CV) -> OpmResult<Self> {
        container_validator.validate_vec(&values)?;
        for v in &values {
            element_validator.validate(v)?;
        }
        Ok(Self {
            values,
            element_validator,
            container_validator,
        })
    }

    /// Returns an immutable reference to the underlying vector.
    ///
    /// # Returns
    ///
    /// * `&Vec<T>` - Reference to the stored values.
    pub const fn get(&self) -> &Vec<T> {
        &self.values
    }

    /// Returns an iterator over the elements.
    ///
    /// This provides immutable access to the underlying values.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.values.iter()
    }

    /// Returns an immutable reference to the underlying vector.
    ///
    /// # Returns
    ///
    /// * `&Vec<T>` - Reference to the stored values.
    ///
    /// # Errors
    /// Returnas an error if the index is out of bounds
    pub fn get_at_index(&self, index: usize) -> OpmResult<&T> {
        if index >= self.values.len() {
            return Err(OpossumError::Other("Index out of bounds".into()));
        }
        Ok(&self.values[index])
    }

    /// Returns the number of elements.
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true if the vector contains no elements.
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns an immutable reference to the first element, if any.
    pub fn first(&self) -> Option<&T> {
        self.values.first()
    }

    /// Returns an immutable reference to the last element, if any.
    pub fn last(&self) -> Option<&T> {
        self.values.last()
    }

    /// Internal helper to mutate the vector safely with rollback.
    ///
    /// # Arguments
    ///
    /// * `f` - Closure performing the mutation.
    /// * `rollback` - Closure to restore the previous state if container validation fails.
    ///
    /// # Returns
    ///
    /// Returns the result of the mutation closure or an error if container validation fails.
    fn mutate_vec_with_rollback<F, R, B>(&mut self, f: F, rollback: B) -> OpmResult<R>
    where
        F: FnOnce(&mut Vec<T>) -> R,
        B: FnOnce(&mut Vec<T>),
    {
        let result = f(&mut self.values);
        if let Err(e) = self.container_validator.validate_vec(&self.values) {
            rollback(&mut self.values); // restore previous state
            return Err(e);
        }
        Ok(result)
    }

    /// Internal helper to mutate the vector safely with rollback.
    ///
    /// # Arguments
    ///
    /// * `f` - Closure performing the mutation.
    /// * `rollback` - Closure to restore the previous state if container validation fails.
    ///
    /// # Returns
    ///
    /// Returns the result of the mutation closure or an error if container validation fails.
    ///
    /// # Errors
    /// Returns an error if mutation fails due to invalid parameters
    pub fn replace(&mut self, index: usize, new_value: T) -> OpmResult<()> {
        if index >= self.values.len() {
            return Err(OpossumError::Other("Index out of bounds".into()));
        }
        self.element_validator.validate(&new_value)?;
        let old_value = self.values[index].clone();
        self.mutate_vec_with_rollback(
            |vec| vec[index] = new_value,
            |vec| vec[index] = old_value, // undo replacement
        )?;
        Ok(())
    }

    /// Appends a new element to the vector after validation.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to append.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the element and container pass validation.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if element validation or container validation fails.
    pub fn push(&mut self, value: T) -> OpmResult<()> {
        self.element_validator.validate(&value)?;
        self.mutate_vec_with_rollback(
            |vec| vec.push(value),
            |vec| {
                let _ = vec.pop();
            }, // undo the push if container validation fails
        )?;
        Ok(())
    }

    /// Removes the last element of the vector.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the operation and container validation succeed.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if the vector is empty or container validation fails.
    pub fn pop(&mut self) -> OpmResult<()> {
        if self.values.is_empty() {
            return Err(OpossumError::Other(
                "Vector is already empty, cannot pop!".into(),
            ));
        }

        let popped = self.values.last().cloned();
        self.mutate_vec_with_rollback(
            std::vec::Vec::pop,
            |vec| {
                if let Some(v) = popped {
                    vec.push(v);
                }
            }, // undo removal
        )?;
        Ok(())
    }

    /// Inserts a new element at the specified index after validation.
    ///
    /// # Arguments
    ///
    /// * `index` - Position to insert the element.
    /// * `value` - The value to insert.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the element and container pass validation.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if index is out of bounds, element validation fails,
    /// or container validation fails after insertion.
    pub fn insert(&mut self, index: usize, value: T) -> OpmResult<()> {
        if index > self.values.len() {
            return Err(OpossumError::Other("Index out of bounds".into()));
        }
        self.element_validator.validate(&value)?;
        self.mutate_vec_with_rollback(
            |vec| vec.insert(index, value),
            |vec| {
                vec.remove(index);
            }, // undo the insert
        )?;
        Ok(())
    }

    /// Applies a closure to each element in the vector.
    ///
    /// Mutations are applied sequentially. After each mutation, the element
    /// is validated immediately. If any element fails validation, all prior
    /// modifications are rolled back.
    ///
    /// Finally, the container validator is run once after all mutations.
    ///
    /// # Errors
    /// Returns an error if validation failed and values are rolled back
    pub fn for_each<F>(&mut self, mut f: F) -> OpmResult<()>
    where
        F: FnMut(&mut T),
    {
        // Track old values for rollback of already-mutated items
        let mut old_values: Vec<(usize, T)> = Vec::with_capacity(self.values.len());

        for (i, elem) in self.values.iter_mut().enumerate() {
            let old = elem.clone();
            f(elem);

            if let Err(e) = self.element_validator.validate(elem) {
                // Roll back all modified elements up to this point
                for (idx, old_val) in old_values {
                    self.values[idx] = old_val;
                }
                self.values[i] = old; // also restore current failing element
                return Err(e);
            }

            old_values.push((i, old));
        }

        // Validate container after all elements are successfully validated
        if let Err(e) = self.container_validator.validate_vec(&self.values) {
            // Rollback all changes if container validation fails
            for (idx, old_val) in old_values {
                self.values[idx] = old_val;
            }
            return Err(e);
        }

        Ok(())
    }

    /// Removes the element at the specified index.
    ///
    /// # Arguments
    ///
    /// * `index` - Position of the element to remove.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if removal and container validation succeed.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if index is out of bounds or container validation fails.
    pub fn remove(&mut self, index: usize) -> OpmResult<()> {
        if index >= self.values.len() {
            return Err(OpossumError::Other("Index out of bounds".into()));
        }
        let removed = self.values[index].clone();
        self.mutate_vec_with_rollback(
            |vec| {
                let _ = vec.remove(index);
            },
            |vec| vec.insert(index, removed), // undo removal
        )?;
        Ok(())
    }

    /// Replaces the entire vector with a new vector after validation.
    ///
    /// # Arguments
    ///
    /// * `new_values` - New vector to replace the current values.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if all elements and the container pass validation.
    ///
    /// # Errors
    ///
    /// Returns `Err(OpossumError)` if any element or container validation fails.
    pub fn set(&mut self, new_values: Vec<T>) -> OpmResult<()> {
        self.container_validator.validate_vec(&new_values)?;
        for v in &new_values {
            self.element_validator.validate(v)?;
        }
        self.values = new_values;
        Ok(())
    }
}

impl<'a, T: Clone, EV: Validate<T>, CV: ValidateVec<T>> IntoIterator
    for &'a ValidatedVec<T, EV, CV>
{
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: Clone, EV: Validate<T>, CV: ValidateVec<T>> Index<usize> for ValidatedVec<T, EV, CV> {
    type Output = T;

    /// Ermöglicht `vec[idx]` für immutable Zugriff.
    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

/// Marker trait used internally by the derive/validation macros to detect
/// whether a type represents a validated value.
///
/// This trait serves as a compile-time helper to check that a type implements
/// some instance of the generic [`Validate<T>`] trait, without needing to know
/// the concrete type parameter `T`.
///
/// In practice, any type that wraps validated data—such as [`Validated`] or
/// [`ValidatedVec`]—will implement this trait automatically.
///
/// # Implementation details
///
/// The [`ValidateTrait`] trait is implemented for:
/// - [`Validated<T, V>`]: any validated single value where `V: Validate<T>`.
/// - [`ValidatedVec<T, V>`]: any validated collection where `V: Validate<T>`
///   and `T: Clone`.
pub trait ValidateTrait {}
impl<T, V: Validate<T>> ValidateTrait for Validated<T, V> {}
impl<T: Clone, EV: Validate<T>, CV: ValidateVec<T>> ValidateTrait for ValidatedVec<T, EV, CV> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic_validators::{AllNotEmpty, AllPositive};

    #[test]
    fn test_validated_new_and_set_is_positive() -> OpmResult<()> {
        let mut v = Validated::new(5, AllPositive)?;
        assert_eq!(*v.get(), 5);

        // Set valid value
        assert!(v.set(10).is_ok());
        assert_eq!(*v.get(), 10);

        // Set invalid value
        assert!(v.set(-2).is_err());
        assert_eq!(*v.get(), 10); // values remains 10
        Ok(())
    }

    #[test]
    fn test_new_valid() -> OpmResult<()> {
        let vec = vec![1, 2, 3];
        let validated = ValidatedVec::new(vec.clone(), AllPositive, AllNotEmpty)?;
        assert_eq!(validated.get(), &vec);
        Ok(())
    }

    #[test]
    fn test_new_invalid_element() {
        let vec = vec![1, -2, 3];
        let val_vec_res = ValidatedVec::new(vec, AllPositive, AllNotEmpty);
        assert!(val_vec_res.is_err());
    }

    #[test]
    fn test_new_invalid_container() {
        let vec: Vec<i32> = vec![];
        let val_vec_res = ValidatedVec::new(vec, AllPositive, AllNotEmpty);
        assert!(val_vec_res.is_err());
    }

    #[test]
    fn test_replace_valid() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2, 3], AllPositive, AllNotEmpty)?;
        validated.replace(1, 5)?;
        assert_eq!(validated.get(), &vec![1, 5, 3]);
        Ok(())
    }

    #[test]
    fn test_replace_invalid_element() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2, 3], AllPositive, AllNotEmpty)?;
        assert!(validated.replace(1, -5).is_err());
        assert_eq!(validated.get(), &vec![1, 2, 3]); // unchanged
        Ok(())
    }

    #[test]
    fn test_replace_invalid_container() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1], AllPositive, AllNotEmpty)?;
        // replace with something valid, container still OK
        validated.replace(0, 10)?;
        assert_eq!(validated.get(), &vec![10]);
        Ok(())
    }

    #[test]
    fn test_replace_index_out_of_bounds() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1], AllPositive, AllNotEmpty)?;
        // replace with something valid, container still OK
        assert!(validated.replace(2, 10).is_err());
        Ok(())
    }

    #[test]
    fn test_push_pop() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2], AllPositive, AllNotEmpty)?;
        validated.push(3)?;
        assert_eq!(validated.get(), &vec![1, 2, 3]);
        validated.pop()?;
        assert_eq!(validated.get(), &vec![1, 2]);
        Ok(())
    }

    #[test]
    fn test_push_invalid_element() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2], AllPositive, AllNotEmpty)?;
        let val_vec_res = validated.push(-1);
        assert!(val_vec_res.is_err());
        assert_eq!(validated.get(), &vec![1, 2]);
        Ok(())
    }

    #[test]
    fn test_pop_invalid_container() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1], AllPositive, AllNotEmpty)?;
        let val_vec_res = validated.pop(); // popping last element → empty
        assert!(val_vec_res.is_err());
        assert_eq!(validated.get(), &vec![1]); // unchanged
        Ok(())
    }

    #[test]
    fn test_insert_remove() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2], AllPositive, AllNotEmpty)?;
        validated.insert(1, 5)?;
        assert_eq!(validated.get(), &vec![1, 5, 2]);
        validated.remove(1)?;
        assert_eq!(validated.get(), &vec![1, 2]);
        Ok(())
    }

    #[test]
    fn test_remove_index_out_of_bounds() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1], AllPositive, AllNotEmpty)?;
        let val_vec_res = validated.remove(2);
        assert!(val_vec_res.is_err());
        Ok(())
    }

    #[test]
    fn test_insert_index_out_of_bounds() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1], AllPositive, AllNotEmpty)?;
        let val_vec_res = validated.insert(2, 0);
        assert!(val_vec_res.is_err());
        let val_vec_res = validated.insert(1, 0);
        assert!(val_vec_res.is_ok());
        Ok(())
    }

    #[test]
    fn test_remove_invalid_container() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1], AllPositive, AllNotEmpty)?;
        let val_vec_res = validated.remove(0); // removing last element → empty
        assert!(val_vec_res.is_err());
        assert_eq!(validated.get(), &vec![1]);
        Ok(())
    }

    #[test]
    fn test_set_valid() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2], AllPositive, AllNotEmpty)?;
        validated.set(vec![3, 4, 5])?;
        assert_eq!(validated.get(), &vec![3, 4, 5]);
        Ok(())
    }

    #[test]
    fn test_set_invalid_element() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2], AllPositive, AllNotEmpty)?;
        let val_vec_res = validated.set(vec![3, -1, 5]);
        assert!(val_vec_res.is_err());
        assert_eq!(validated.get(), &vec![1, 2]);
        Ok(())
    }

    #[test]
    fn test_set_invalid_container() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2], AllPositive, AllNotEmpty)?;
        let val_vec_res = validated.set(vec![]);
        assert!(val_vec_res.is_err());
        assert_eq!(validated.get(), &vec![1, 2]); // unchanged
        Ok(())
    }

    #[test]
    fn test_insert_boundaries() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2], AllPositive, AllNotEmpty)?;

        // Insert at beginning
        validated.insert(0, 5)?;
        assert_eq!(validated.get(), &vec![5, 1, 2]);

        // Insert at end
        validated.insert(validated.get().len(), 6)?;
        assert_eq!(validated.get(), &vec![5, 1, 2, 6]);
        Ok(())
    }

    #[test]
    fn test_remove_boundaries() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2, 3], AllPositive, AllNotEmpty)?;

        // Remove first element
        validated.remove(0)?;
        assert_eq!(validated.get(), &vec![2, 3]);

        // Remove last element
        validated.remove(validated.get().len() - 1)?;
        assert_eq!(validated.get(), &vec![2]);
        Ok(())
    }

    #[test]
    fn test_sequential_mutations_with_partial_failures() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2, 3], AllPositive, AllNotEmpty)?;

        // First replacement succeeds
        validated.replace(0, 5)?;
        assert_eq!(validated.get(), &vec![5, 2, 3]);

        // Second replacement fails element validation
        assert!(validated.replace(1, -1).is_err());
        assert_eq!(validated.get(), &vec![5, 2, 3]); // rollback works

        // Push succeeds
        validated.push(6)?;
        assert_eq!(validated.get(), &vec![5, 2, 3, 6]);

        // Remove fails container validation (vector must not be empty)
        let mut small = ValidatedVec::new(vec![1], AllPositive, AllNotEmpty)?;
        assert!(small.remove(0).is_err());
        assert_eq!(small.get(), &vec![1]);
        Ok(())
    }

    #[test]
    fn test_multiple_replacements() -> OpmResult<()> {
        let mut validated = ValidatedVec::new(vec![1, 2, 3], AllPositive, AllNotEmpty)?;

        validated.replace(0, 10)?;
        validated.replace(2, 20)?;
        assert_eq!(validated.get(), &vec![10, 2, 20]);

        // Attempt invalid replacement in middle
        assert!(validated.replace(1, -5).is_err());
        assert_eq!(validated.get(), &vec![10, 2, 20]); // only failed replacement rolled back
        Ok(())
    }
}
