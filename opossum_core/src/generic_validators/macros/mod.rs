mod validator_parser;
mod validated;
mod validated_vec;

/// Implements the [`Validate`] trait for a given validator type, function, and value type.
///
/// This macro provides a concise way to define validation logic without manually
/// writing repetitive boilerplate for error handling and `Result` wrapping.
///
/// The generated implementation automatically:
/// - Calls the provided function `($func)` with `(&self, &value)`.
/// - Returns `Ok(())` if the function returns `true`.
/// - Returns an [`OpossumError::Other`] with a descriptive message if the function returns `false`.
///
/// # Parameters
/// - `$validator`: The path to the validator type implementing [`Validate`].
/// - `$func`: A function or closure reference that performs the validation check.
/// - `$t`: The type of value to validate.
///
/// # Notes
/// - This macro assumes that `$func` has the signature `fn(&Self, &T) -> bool`.
/// - The generated error message includes the stringified name of `$func`.
/// - Useful for scalar or single-value validators.  
///   For collection-level validation, see [`impl_vec_validator!`].
#[macro_export]
macro_rules! impl_validator {
    ($validator:path, $func:expr,  $t:ty) => {
        impl $crate::generic_validators::Validate<$t> for $validator {
            fn validate(&self, value: &$t) -> $crate::error::OpmResult<()> {
                if $func(&self, &value) {
                    Ok(())
                } else {
                    Err($crate::error::OpossumError::Other(format!(
                        "Value must satisfy {}",
                        stringify!($func)
                    )))
                }
            }
        }
    };
}

/// Implements the [`ValidateVec`] trait for a given validator type, function, and element type.
///
/// This macro defines collection-level validation logic in a compact form, automatically
/// handling result wrapping and error reporting for vector-based checks.
///
/// The generated implementation:
/// - Calls the provided function `($func)` with `(&self, &values)`.
/// - Returns `Ok(())` if the function returns `true`.
/// - Returns an [`OpossumError::Other`] with a descriptive message if the function returns `false`.
///
/// # Parameters
/// - `$validator`: The path to the validator type implementing [`ValidateVec`].
/// - `$func`: A function or closure reference performing validation across the entire vector.
/// - `$t`: The type of each element in the vector being validated.
///
/// # Notes
/// - This macro assumes `$func` has the signature `fn(&Self, &Vec<T>) -> bool`.
/// - Intended for validators operating on vector-wide conditions
///   (e.g. "non-empty", "sum > 0", "no NaNs").
/// - For per-element validation, use [`impl_validator!`].
#[macro_export]
macro_rules! impl_vec_validator {
    ($validator:path, $func:expr,  $t:ty) => {
        impl $crate::generic_validators::ValidateVec<$t> for $validator {
            fn validate_vec(&self, values: &Vec<$t>) -> $crate::error::OpmResult<()> {
                if $func(&self, &values) {
                    Ok(())
                } else {
                    Err($crate::error::OpossumError::Other(format!(
                        "All values must satisfy {}",
                        stringify!($func)
                    )))
                }
            }
        }
    };
}
