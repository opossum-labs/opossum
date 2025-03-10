use proc_macro::TokenStream;
use quote::{quote, TokenStreamExt};
use syn::{parse_macro_input, AttrStyle, ItemStruct, LitStr};

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
