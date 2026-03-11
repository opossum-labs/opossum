use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Data, DeriveInput, Fields, GenericArgument, Meta, PathArguments, Type, TypePath,
    parse_macro_input,
};

/// Helper function to check if a field or variant has `#[validate(skip)]` attribute.
/// Returns true if the field/variant should be skipped for validation.
fn has_skip(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if a.path().is_ident("validate") {
            if let Meta::List(meta_list) = &a.meta {
                meta_list.tokens.to_string().contains("skip")
            } else {
                false
            }
        } else {
            false
        }
    })
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
pub fn impl_derive_ensure_validated(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a Rust syntax tree
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Vectors to accumulate compile-time errors and marker checks
    let mut errors = Vec::new();
    let mut checks = Vec::new();

    // Process the struct or enum
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                // Process named fields
                for field in &fields.named {
                    process_field(field, &name.to_string(), &mut errors, &mut checks);
                }
            }
            Fields::Unnamed(fields) => {
                // Process tuple struct fields
                for field in &fields.unnamed {
                    process_field(field, &name.to_string(), &mut errors, &mut checks);
                }
            }
            Fields::Unit => {
                // Unit structs have no fields → nothing to check
            }
        },
        Data::Enum(data_enum) => {
            for variant in &data_enum.variants {
                let variant_name = &variant.ident;

                // Skip variants with `#[validate(skip)]`
                if has_skip(&variant.attrs) {
                    continue;
                }

                match &variant.fields {
                    Fields::Named(fields) => {
                        for field in &fields.named {
                            process_field(
                                field,
                                &format!("{name}.{variant_name}"),
                                &mut errors,
                                &mut checks,
                            );
                        }
                    }
                    Fields::Unnamed(fields) => {
                        for field in &fields.unnamed {
                            process_field(
                                field,
                                &format!("{name}.{variant_name}"),
                                &mut errors,
                                &mut checks,
                            );
                        }
                    }
                    Fields::Unit => {
                        // Unit variants have no fields → nothing to check
                    }
                }
            }
        }
        Data::Union(_) => {
            return quote! {
                compile_error!("EnsureValidated can only be derived for enums or structs!");
            }
            .into();
        }
    }

    // Generate the impl block with marker constant
    let marker_impl = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// Marker constant to indicate that this type has been validated.
            pub const _ENSURE_VALIDATED_MARKER: () = ();
        }
    };

    // Quote blocks for compile-time errors and marker checks
    let errors_block = quote! { #(#errors)* };
    let checks_block = quote! { #(#checks)* };

    // Generate the final output tokens
    let output = quote! {
        #marker_impl
        const _: () = {
            #errors_block
            #checks_block
        };
    };

    output.into()
}

/// Checks a single field for validation compliance.
///
/// Rules:
/// - Fields with #[validate(skip)] are ignored.
/// - Fields that are Validated/ValidatedVec must implement `ValidateTrait`.
/// - Other structs/enums must have the `_ENSURE_VALIDATED_MARKER`.
fn process_field(
    field: &syn::Field,
    parent: &str,
    errors: &mut Vec<proc_macro2::TokenStream>,
    checks: &mut Vec<proc_macro2::TokenStream>,
) {
    let field_name = field
        .ident
        .as_ref()
        .map_or_else(|| "<unnamed>".into(), std::string::ToString::to_string);

    // Skip fields marked with #[validate(skip)]
    if has_skip(&field.attrs) {
        return;
    }

    let ty = &field.ty;

    // Case 1: Field contains Validated/ValidatedVec → check ValidateTrait
    if contains_validated(ty) {
        checks.push(quote! {
                let _ : fn() = || {
            // The type itself must implement the trait
            fn _check<T: ValidateTrait>() {}
            _check::<#ty>();
        };
            });
        return;
    }

    // Case 2: Field is a struct or enum → check its _ENSURE_VALIDATED_MARKER
    if let Type::Path(TypePath { path, .. }) = ty {
        checks.push(quote! {
            let _ = #path :: _ENSURE_VALIDATED_MARKER;
        });

        return;
    }

    // Case 3: Otherwise, emit compile-time error
    errors.push(quote! {
        compile_error!(concat!(
            "Field `", #field_name, "` in `", #parent,
            "` must be either a Validated<T, V> / ValidatedVec<T, V> with Validate implemented,",
            " or a struct/enum annotated with #[derive(EnsureValidated)]."
        ));
    });
}

/// Recursively checks whether a type is `Validated<_>` or `ValidatedVec<_>`.
///
/// Supports:
/// - Path types with generics
/// - References (`&T` / `&mut T`)
/// - Tuples
/// - Arrays, parenthesis, and grouped types
fn contains_validated(ty: &Type) -> bool {
    match ty {
        Type::Path(tp) => {
            // Construct the full path string for matching
            let full_path: String = tp
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");

            // Accept the canonical path or any known re-exports
            if matches!(
                full_path.as_str(),
                "opossum_core::generic_validators::Validated"
                    | "opossum_core::generic_validators::ValidatedVec"
                    | "opossum_core::Validated"
                    | "opossum_core::ValidatedVec"
                    | "Validated"
                    | "ValidatedVec"
            ) || full_path.contains("Validated")
            {
                return true;
            }
            // Check all segments for nested generics
            for seg in &tp.path.segments {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    for arg in &ab.args {
                        if let GenericArgument::Type(inner_ty) = arg
                            && contains_validated(inner_ty)
                        {
                            return true;
                        }
                    }
                }
            }
            false
        }

        Type::Macro(m) => {
            let path_str = m
                .mac
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");

            if matches!(
                path_str.as_str(),
                "opossum_core::generic_validators::impl_macro::validated_type"
                    | "generic_validators::impl_macro::validated_type"
                    | "impl_macro::validated_type"
                    | "validated_type"
                    | "opossum_core::generic_validators::impl_macro::validated_vec_type"
                    | "generic_validators::impl_macro::validated_vec_type"
                    | "impl_macro::validated_vec_type"
                    | "validated_vec_type"
            ) {
                // treat as Validated
                return true;
            }

            false
        }

        Type::Reference(r) => contains_validated(&r.elem),
        Type::Tuple(t) => t.elems.iter().any(contains_validated),
        Type::Array(a) => contains_validated(&a.elem),
        Type::Group(g) => contains_validated(&g.elem),
        Type::Paren(p) => contains_validated(&p.elem),
        _ => false,
    }
}
