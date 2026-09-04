use crate::opm_node::impl_derive_opm_node;
use proc_macro::TokenStream;

mod ensure_validated;
mod opm_node;
mod validate_numeric;

/// Add basic functions and traits for an optical node.
///
/// # Panics
///
/// Panics if the arguments cannot be sucessfully parsed.
#[proc_macro_derive(OpmNode, attributes(opm_node, manual_analyzable))]
pub fn derive_opm_node(input: TokenStream) -> TokenStream {
    impl_derive_opm_node(input)
}

/// Derive macro `EnsureValidated`.
///
/// This macro:
/// - Can be applied to structs and enums.
/// - Generates a marker constant `_ENSURE_VALIDATED_MARKER` inside an `impl`
///   block for the type. Other types can reference this marker to enforce
///   that nested types are also validated.
/// - Checks that all fields or enum variants are either:
///   - `Validated<_>`
///   - `ValidatedVec<_>`
///   - Or a type that also derives `EnsureValidated`.
/// - Fields or variants marked with `#[validate(skip)]` are ignored.
///
/// ## Example
///
/// ```
/// use opm_macros_lib::EnsureValidated;
/// use opossum_core::generic_validators::{ValidateTrait, AllNotEmpty, Validated, ValidatedVec, AllPositive};
///
/// #[derive(EnsureValidated)]
/// struct Address {
///     street: Validated<String, AllNotEmpty>,
///     city: Validated<String, AllNotEmpty>,
/// }
///
/// #[derive(EnsureValidated)]
/// enum Contact {
///     Email(Validated<String, AllNotEmpty>),
///     Phone(Validated<String, AllNotEmpty>),
///
///     // Unit variant, automatically ignored.
///     Unknown,
///
///     // Explicitly skipped variant.
///     #[validate(skip)]
///     Temporary,
/// }
///
/// #[derive(EnsureValidated)]
/// struct User {
///     name: Validated<String, AllNotEmpty>,
///     address: Address,               // Nested type with its own EnsureValidated
///     contact: Contact,               // Nested type
///     misc_data: ValidatedVec<i32, AllPositive, AllNotEmpty>, // Custom validated vector
///
///     #[validate(skip)]
///     cached_value: Option<String>,   // Skipped from validation
/// }
/// ```
///
/// If a field or variant fails the validation rules (e.g., plain `String` without validation),
/// a **compile-time error** will be raised, ensuring validation consistency.
#[proc_macro_derive(EnsureValidated, attributes(validate))]
pub fn derive_ensure_validated(input: TokenStream) -> TokenStream {
    ensure_validated::impl_derive_ensure_validated(input)
}

/// Automatically implements numeric validation traits for a struct using a declarative `#[rule(...)]` attribute.
///
/// # Overview
///
/// This procedural macro generates `Validate` and `ValidateVec` implementations for numeric types,
/// supporting validation of:
///
/// - single numeric values (`T`)
/// - numeric tuples (`(T, T)`)
/// - 2D points (`Point2<T>`)
/// - numeric ranges (`Range<T>`)
///
/// depending on whether the target type is generic or concrete.
///
/// The generated implementations use the provided validation rule function, either from:
/// - the implementing struct itself (`on = "self"`), or
/// - a static method on the `NumLike` trait (`on = "numlike"`, the default).
///
/// The validation logic can be fine-tuned using parameters in the `#[rule(...)]` attribute.
///
/// # Attribute Syntax
///
/// ```
/// #[rule(
///     rule_fn_name,                     // name of the validation function (required)
///     message = "Custom error message", // optional custom error message
///     target = "x" | "y" | "both",      // which part to validate (default: "both")
///     mode = "all" | "any",             // whether all or any checks must pass (default: "all")
///     on = "self" | "numlike"           // call rule on self or NumLike trait (default: "numlike")
/// )]
/// ```
///
/// # Behavior by Type
///
/// - **`Validate<T>`**
///   Validates a single numeric value.
///   Example: `rule_fn(&val)`
///
/// - **`Validate<(T, T)>`**
///   Validates a tuple of two values of the **same type `T`**.
///   The macro explicitly enforces `(T, T)` for generics — mixed-type tuples like `(f32, i32)` are **not supported**.
///
/// - **`Validate<Point2<T>>`**
///   Validates both coordinates of a `Point2<T>`.
///   `target = "x"`, `"y"`, or `"both"` controls which component(s) are checked.
///
/// - **`Validate<Range<T>>`**
///   Validates both ends of a numeric range (`Range<T>`).
///   Uses the same `target` and `mode` semantics as `Point2<T>`.
///
/// - **`ValidateVec<T>` and `ValidateVec<(T, T)>`**
///   Vectorized versions of the above traits, iterating over all elements.
///   Controlled by `mode = "all"` (all must pass) or `mode = "any"` (at least one passes).
///
/// # Notes
///
/// - The `on = "self"` flag makes the validator call a method **on the struct instance** (e.g. `self.is_positive(val)`),
///   instead of a static method from `NumLike`.
/// - When validating tuples with generics, both elements must have the **same type** `T`.
///   If mixed types are needed (e.g. `(f32, i32)`), use a **non-generic** validator struct instead.
/// - For `Point2` and `Range`, the `target` parameter determines which field(s) are validated.
/// - The macro assumes that `NumLike`, `Validate`, `ValidateVec`, `Point2`, `Range`, and `OpmResult` are in scope.
///
/// # Generated Traits
///
/// The macro implements combinations of:
/// ```ignore
/// impl<T: NumLike> Validate<T> for MyValidator<T> { ... }
/// impl<T: NumLike> Validate<(T, T)> for MyValidator<T> { ... }
/// impl<T: NumLike> Validate<Point2<T>> for MyValidator<T> { ... }
/// impl<T: NumLike> Validate<Range<T>> for MyValidator<T> { ... }
/// impl<T: NumLike> ValidateVec<T> for MyValidator<T> { ... }
/// impl<T: NumLike> ValidateVec<(T, T)> for MyValidator<T> { ... }
/// impl<T: NumLike> ValidateVec<Point2<T>> for MyValidator<T> { ... }
/// ```
///
/// # Panics
///
/// The macro will panic at compile time if:
/// - The `#[rule(...)]` attribute is missing or incorrectly formatted.
/// - The specified rule function name cannot be parsed.
#[proc_macro_derive(ValidateNumeric, attributes(rule))]
pub fn derive_validate_numeric(input: TokenStream) -> TokenStream {
    validate_numeric::impl_derive_validate_numeric(input)
}
