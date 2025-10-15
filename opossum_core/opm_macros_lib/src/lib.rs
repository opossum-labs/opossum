use proc_macro::TokenStream;
use quote::{TokenStreamExt, quote};
use syn::{parse_macro_input, AttrStyle, Data, DeriveInput, ItemStruct, LitStr, Meta, NestedMeta};

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
        .find(|a| a.path.is_ident("opm_node") && a.style == AttrStyle::Outer);
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


#[proc_macro_derive(Validate, attributes(validate))]
pub fn derive_validate(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = if let Data::Struct(s) = &input.data {
        &s.fields
    } else {
        return syn::Error::new_spanned(name, "Validate only supports structs")
            .to_compile_error()
            .into();
    };

    // Umgeschriebene Felddefinitionen (f64 -> Validated<f64, IsPositive>)
    let new_fields = fields.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;

        // Finde #[validate(...)] Attribut
        if let Some(attr) = f.attrs.iter().find(|a| a.path.is_ident("validate")) {
            let meta = attr.parse_meta().unwrap();
            if let Meta::List(list) = meta {
                let validators: Vec<_> = list.nested.iter().map(|n| match n {
                    NestedMeta::Meta(Meta::Path(p)) => quote! { #p },
                    _ => quote! {},
                }).collect();

                if validators.len() == 1 {
                    let v = &validators[0];
                    quote! {
                        #ident: Validated<#ty, #v>
                    }
                } else {
                    // mehrere Validatoren -> kombiniere über AndValidator
                    let first = validators[0].clone();
                    let chain = validators[1..].iter().fold(first, |acc, v| {
                        quote! { AndValidator::<#ty, _, _>::new(#acc, #v) }
                    });

                    quote! {
                        #ident: Validated<#ty, _> // generischer Platzhalter, du kannst hier den Typen kombinieren
                    }
                }
            } else {
                quote! { #ident: #ty }
            }
        } else {
            quote! { #ident: #ty }
        }
    });

    // Eine Hilfsfunktion generieren, um aus normalen Werten eine Instanz zu bauen
    let new_params = fields.iter().map(|f| {
        let ident = &f.ident;
        let ty = &f.ty;
        quote! { #ident: #ty }
    });

    let new_inits = fields.iter().map(|f| {
        let ident = &f.ident;

        if let Some(attr) = f.attrs.iter().find(|a| a.path.is_ident("validate")) {
            let meta = attr.parse_meta().unwrap();
            if let Meta::List(list) = meta {
                let validator = list.nested.first().unwrap();
                quote! {
                    #ident: Validated::new(#ident, #validator)?
                }
            } else {
                quote! { #ident }
            }
        } else {
            quote! { #ident }
        }
    });

    let expanded = quote! {
        pub struct #name {
            #(#new_fields),*
        }

        impl #name {
            pub fn new(#(#new_params),*) -> crate::error::OpmResult<Self> {
                Ok(Self {
                    #(#new_inits),*
                })
            }
        }
    };

    TokenStream::from(expanded)
}
// pub fn derive_validate(input: TokenStream) -> TokenStream {
//     let input = parse_macro_input!(input as DeriveInput);
//     let name = &input.ident;

//     let fields = if let syn::Data::Struct(data) = &input.data {
//         &data.fields
//     } else {
//         return syn::Error::new_spanned(&input, "Validate only supports structs")
//             .to_compile_error()
//             .into();
//     };

//     let validations = fields.iter().filter_map(|f| {
//         let ident = &f.ident;
//         let attr = f.attrs.iter().find(|a| a.path.is_ident("validate"))?;

//         let meta = attr.parse_meta().ok()?;
//         if let Meta::List(list) = meta {
//             let validators = list.nested.iter().map(|n| match n {
//                 NestedMeta::Meta(Meta::Path(p)) => quote! { #p },
//                 _ => quote! {},
//             });

//             Some(quote! {
//                 #(
//                     #validators.validate(&self.#ident)?;
//                 )*
//             })
//         } else {
//             None
//         }
//     });

//     let expanded = quote! {
//         impl #name {
//             pub fn validate(&self) -> crate::error::OpmResult<()> {
//                 #(#validations)*
//                 Ok(())
//             }
//         }
//     };

//     TokenStream::from(expanded)
// }
// pub fn derive_validate(input: TokenStream) -> TokenStream {
//     let input = parse_macro_input!(input as DeriveInput);
//     let struct_name = &input.ident;

//     let fields = if let Data::Struct(s) = &input.data {
//         &s.fields
//     } else {
//         return syn::Error::new_spanned(struct_name, "Validate only works for structs")
//             .to_compile_error()
//             .into();
//     };

//     let inits = fields.iter().map(|f| {
//         let ident = &f.ident;
//         let ty = &f.ty;

//         let validator_attr = f.attrs.iter().find(|a| a.path.is_ident("validate"));

//         if let Some(attr) = validator_attr {
//             let meta = attr.parse_meta().unwrap();
//             if let Meta::List(list) = meta {
//                 // Split NestedMeta auf &&-Kette
//                 let validators: Vec<_> = list.nested.iter().filter_map(|n| {
//                     if let NestedMeta::Meta(Meta::Path(p)) = n {
//                         Some(quote! { #p })
//                     } else {
//                         None
//                     }
//                 }).collect();

//                 // Rekursive Verschachtelung von AndValidator
//                 let nested = validators.iter().rev().cloned().reduce(|a, b| {
//                     quote! { AndValidator::<#ty, #b, #a>::new(#b, #a) }
//                 }).unwrap();

//                 quote! {
//                     #ident: Validated::<#ty, _>::new(Default::default(), #nested).unwrap()
//                 }
//             } else {
//                 quote! { #ident: Default::default() }
//             }
//         } else {
//             quote! { #ident: Default::default() }
//         }
//     });

//     let expanded = quote! {
//         impl Default for #struct_name {
//             fn default() -> Self {
//                 Self {
//                     #(#inits),*
//                 }
//             }
//         }
//     };

//     TokenStream::from(expanded)
// }
