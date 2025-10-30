/// Constructs a [`ValidatedVec`] instance from a vector and validator expressions.
///
/// This macro provides a concise, expressive way to create a validated vector
/// that checks both individual elements and the vector as a whole, using your
/// validator DSL syntax (e.g. `XNormal && YFinite && AllPositive`).
///
/// The macro automatically splits the provided validator sequence at the first comma:
/// - the **first part** defines the *element-level validator*,  
/// - the **second part** defines the *container-level validator*.
///
/// Validation is done on creation and on setting single parameters.
///
/// # Notes
/// - The validators can use logical operators (`&&`, `||`, `!`) as defined by your validator DSL.
/// - The macro returns a `Result<ValidatedVec<...>, ValidationError>` — so call `.unwrap()`, `.expect()`, or handle the error.
/// - For the type-level equivalent, see [`validated_vec_type!`].
#[macro_export]
macro_rules! validated_vec {
    ($value:expr, $($rest:tt)+) => {
        $crate::validated_vec!(@split $value, [] $($rest)+)
    };

    (@split $value:expr, [$($accum:tt)*] , $($tail:tt)+) => {{
        let elem_validator = $crate::validator_expr!($($accum)*);
        let cont_validator = $crate::validator_vec_expr!($($tail)+);
        $crate::generic_validators::ValidatedVec::new(
            $value,
            elem_validator,
            cont_validator
        )
    }
    };

    (@split $value:expr, [$($accum:tt)*] $head:tt $($tail:tt)*) => {
        $crate::validated_vec!(@split $value, [$($accum)* $head] $($tail)*)
    };
}

/// Expands to the **type** of a [`ValidatedVec`] given an inner vector type and validator expressions.
///
/// This macro defines the *type* corresponding to a validated vector — it mirrors
/// [`validated_vec!`] but produces a type rather than constructing an instance.
///
/// It’s designed for use in struct definitions, type aliases, and generic type parameters.
///
/// # Notes
/// - This macro only expands to a *type*; no runtime validation occurs.
/// - It’s ideal for struct fields or type aliases where validation should be enforced at construction time.
/// - Combine with [`validated_vec!`] in your `impl Default` or builder patterns for clean, type-safe validation.
#[macro_export]
macro_rules! validated_vec_type {
    (Vec< $inner:ty >, $($rest:tt)+) => {
        $crate::validated_vec_type!(@split $inner [] $($rest)+)
    };

    (@split $inner:ty [$($accum:tt)*] , $($tail:tt)+) => {
        $crate::generic_validators::ValidatedVec<
            $inner,
            $crate::validator_type_expr!($inner; $($accum)*),
            $crate::validator_vec_type_expr!($inner; $($tail)+)
        >
    };

    (@split $inner:ty [$($accum:tt)*] $head:tt $($tail:tt)*) => {
        $crate::validated_vec_type!(@split $inner [$($accum)* $head] $($tail)*)
    };
}
