use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, AttrStyle, ItemStruct, LitStr};

pub fn impl_derive_opm_node(input: TokenStream) -> TokenStream {
    let struct_input = parse_macro_input!(input as ItemStruct);

    // Delegate execution to an inner helper returning a syn::Result
    match expand_opm_node(&struct_input) {
        Ok(tokens) => tokens.into(),
        // Convert the syn::Error into a compile_error! token stream
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_opm_node(struct_input: &ItemStruct) -> syn::Result<TokenStream2> {
    let struct_name = &struct_input.ident;

    // Check if the struct has the #[manual_analyzable] helper attribute
    let has_manual_analyzable = struct_input
        .attrs
        .iter()
        .any(|a| a.path().is_ident("manual_analyzable"));

    // Conditionally implement Analyzable unless manual implementation is flagged
    let analyzable_impl = if has_manual_analyzable {
        quote! {}
    } else {
        quote! {
            impl Analyzable for #struct_name {
                fn clone_analyzable(&self) -> ::std::boxed::Box<dyn Analyzable> {
                    ::std::boxed::Box::new(self.clone())
                }
            }
        }
    };

    // Parse the optional #[opm_node("...")] attribute cleanly without panicking
    let dottable_impl = if let Some(attr) = struct_input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("opm_node") && a.style == AttrStyle::Outer)
    {
        // Parsing errors are propagated with '?' instead of causing a panic
        let color_lit: LitStr = attr.parse_args()?;
        quote! {
            use crate::reporting::Dottable;
            impl Dottable for #struct_name {
                fn node_color(&self) -> &'static str {
                    #color_lit
                }
            }
        }
    } else {
        quote! {}
    };

    // Combine all generated trait implementations
    let expanded = quote! {
        use crate::{
            analyzers::Analyzable,
            core_optics::optic_node::{Alignable, LIDT},
        };

        #analyzable_impl
        #dottable_impl

        impl Alignable for #struct_name {}
        impl LIDT for #struct_name {}

        // Automatically implement the attribute access trait
        impl crate::core_optics::node_attr::HasNodeAttr for #struct_name {
            fn node_attr(&self) -> &crate::core_optics::NodeAttr {
                &self.node_attr
            }
            fn node_attr_mut(&mut self) -> &mut crate::core_optics::NodeAttr {
                &mut self.node_attr
            }
        }

        // Automatically implement the downcasting trait
        impl crate::core_optics::optic_node::OpticNodeAny for #struct_name {
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self
            }
        }
    };

    Ok(expanded)
}