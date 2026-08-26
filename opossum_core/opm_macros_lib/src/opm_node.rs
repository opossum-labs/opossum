use proc_macro::TokenStream;
use quote::{TokenStreamExt, quote};
use syn::{AttrStyle, ItemStruct, LitStr, parse_macro_input};

pub fn impl_derive_opm_node(input: TokenStream) -> TokenStream {
    let struct_input = parse_macro_input!(input as ItemStruct);
    let struct_name = &struct_input.ident;

    // Check if the struct has the #[manual_analyzable] helper attribute
    let has_manual_analyzable = struct_input
        .attrs
        .iter()
        .any(|a| a.path().is_ident("manual_analyzable"));

    // Generate Analyzable implementation only if not marked as manual
    let analyzable_impl = if has_manual_analyzable {
        quote! {}
    } else {
        quote! {
            impl Analyzable for #struct_name {
                fn clone_analyzable(&self) -> std::sync::Arc<std::sync::Mutex<dyn Analyzable>> {
                    std::sync::Arc::new(std::sync::Mutex::new(self.clone()))
                }
            }
        }
    };

    let mut code = quote! {
        use crate::{
            analyzers::Analyzable,
            core_optics::optic_node::{Alignable, LIDT},
        };

        #analyzable_impl
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
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    };

    let attrs = struct_input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("opm_node") && a.style == AttrStyle::Outer);
    if let Some(attr) = attrs {
        let args: LitStr = attr.parse_args().unwrap();
        let arg = args.value();
        let dottable = quote! {
            use crate::reporting::Dottable;
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
