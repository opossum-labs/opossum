use proc_macro::TokenStream;

mod ensure_validated;
mod opm_node;

/// Add basic functions and traits for an optical node.
///
/// # Panics
///
/// Panics if the arguments cannot be sucessfully parsed.
#[proc_macro_derive(OpmNode, attributes(opm_node))]
pub fn derive_opm_node(input: TokenStream) -> TokenStream {
    opm_node::impl_derive_opm_node(input)
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
/// ```rust,ignore
/// use my_macros::EnsureValidated;
///
/// // Suppose this is your validated wrapper type.
/// struct Validated<T, V>(T, V);
/// struct ValidatedVec<T, V>(Vec<T>, V);
/// struct AllNotEmpty;
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
///     misc_data: ValidatedVec<i32, AllNotEmpty>, // Custom validated vector
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
