use proc_macro::TokenStream;
use quote::{TokenStreamExt, quote};
use syn::{AttrStyle, ItemStruct, LitStr, parse_macro_input};

pub fn impl_derive_opm_node(input: TokenStream) -> TokenStream {
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
