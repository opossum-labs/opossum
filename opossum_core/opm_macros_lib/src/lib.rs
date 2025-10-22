use proc_macro::TokenStream;
use quote::{TokenStreamExt, quote};
use syn::{
    AttrStyle, Data, DeriveInput, Fields, GenericArgument, ItemStruct, LitStr, Meta, PathArguments,
    Type, TypePath, parse_macro_input,
};

// #[proc_macro_attribute]
// pub fn opm_node(_args: TokenStream, input: TokenStream) -> TokenStream {
//   // 1. Use syn to parse the args & input tokens into a syntax tree.
//   // 2. Generate new tokens based on the syntax tree. This will replace whatever `item` is
//   //    annotated w/ this attribute proc macro.
//   // 3. Return the generated tokens.
//   let cloned_input=input.clone();
//   let my_input=parse_macro_input!(input as ItemStruct);
//   let fields=my_input.fields;
//   let members: Vec<String> = fields.into_iter().map(|i| i.ident).flatten().map(|i| i.to_string()).collect();
//   if members.contains(&"node_attr".to_string()) {
//     eprintln!("contains node_attr");
//   }
//   cloned_input
// }

/// Add basic functions and traits for an optical node.
///
/// # Panics
///
/// Panics if the arguments cannot be sucessfully parsed.
#[proc_macro_derive(OpmNode, attributes(opm_node))]
pub fn derive_opm_node(input: TokenStream) -> TokenStream {
    let struct_input = parse_macro_input!(input as ItemStruct);
    let struct_name = struct_input.ident;

    let mut code = quote! {
        use crate::{analyzers::Analyzable,
            optic_node::{Alignable, LIDT}};
        impl Analyzable for #struct_name {}
        impl Alignable for #struct_name {}
        impl LIDT for #struct_name {}
    };
    let attrs = struct_input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("opm_node") && a.style == AttrStyle::Outer);
    if let Some(attr) = attrs {
        let args: LitStr = attr.parse_args().unwrap();
        let arg = args.value();
        let dottable = quote! {
            use crate::dottable::Dottable;
            impl Dottable for #struct_name {
                fn node_color(&self) -> &'static str {
                    #arg
                }
            }
        };
        code.append_all(dottable);
    }
    code.into()
}

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
pub fn ensure_validated_derive(item: TokenStream) -> TokenStream {
    // Parse the input tokens into a Rust syntax tree
    let input = parse_macro_input!(item as DeriveInput);
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
            ) {
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
        Type::Reference(r) => contains_validated(&r.elem),
        Type::Tuple(t) => t.elems.iter().any(contains_validated),
        Type::Array(a) => contains_validated(&a.elem),
        Type::Group(g) => contains_validated(&g.elem),
        Type::Paren(p) => contains_validated(&p.elem),
        _ => false,
    }
}
