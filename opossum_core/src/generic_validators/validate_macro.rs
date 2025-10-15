use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Meta, NestedMeta, Data, Fields};
use quote::quote;

/// Proc-Macro: #[derive(Validate)] + #[validate(...)]
#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = input.ident;

    // Nur Structs erlauben
    let fields = if let Data::Struct(s) = input.data {
        s.fields
    } else {
        return syn::Error::new_spanned(struct_name, "Validate can only be derived for structs")
            .to_compile_error()
            .into();
    };

    // Jedes Feld parsen
    let field_inits = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;

        let validator = f.attrs.iter()
            .find(|a| a.path.is_ident("validate"))
            .map(|attr| {
                attr.parse_meta().unwrap() // Meta::List
            });

        if let Some(Meta::List(list)) = validator {
            // Liste von NestedMeta → AndValidator Kette
            let validators: Vec<_> = list.nested.iter().map(|n| match n {
                NestedMeta::Meta(Meta::Path(p)) => quote! { #p },
                NestedMeta::Meta(Meta::List(l)) => quote! { #l },
                _ => quote! {},
            }).collect();

            let first = validators[0].clone();
            let chain = validators[1..].iter().fold(first, |acc, v| {
                quote! { AndValidator::<#ty, _, _>::new(#acc, #v) }
            });

            quote! {
                #name: Validated::<#ty, _>::new(Default::default(), #chain).unwrap()
            }

        } else {
            // Kein Validator → Default
            quote! { #name: Default::default() }
        }
    });

    let expanded = quote! {
        impl Default for #struct_name {
            fn default() -> Self {
                Self {
                    #(#field_inits),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
