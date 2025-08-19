mod constant_transmission_editor;
mod spectral_transmission_editor;

use crate::components::node_editor::{
    accordion::ElementList,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    optical_node_editor::properties_editor::use_set_node_change_property,
};
pub use constant_transmission_editor::ConstantFilterTypeEditor;
use dioxus::prelude::*;
use opossum_backend::{DefaultFromName, FilterTypeBuilder, Property};
pub use spectral_transmission_editor::SpectralFilterTypeEditor;

#[component]
pub fn FilterTypeEditor(
    filter_type_builder: FilterTypeBuilder,
    property_key: String,
    property: Property,
) -> Element {
    let filter_type_builder_sig = use_signal(|| filter_type_builder.clone());
    use_context_provider(|| property);

    use_set_node_change_property(&property_key, filter_type_builder, filter_type_builder_sig);

    let mut element_list: Vec<Result<VNode, RenderError>> = vec![rsx! {
    FilterTypeSelector {filter_type_builder_sig}}];

    match &*filter_type_builder_sig.read() {
        FilterTypeBuilder::Constant(transmission) => element_list.push(rsx! {
            ConstantFilterTypeEditor {
                transmission: *transmission,
                builder_sig: filter_type_builder_sig,
            }
        }),
        FilterTypeBuilder::Spectrum(spectral_filter_builder) => element_list.push(rsx! {
            SpectralFilterTypeEditor {
                spectral_filter_builder: spectral_filter_builder.clone(),
                builder_sig: filter_type_builder_sig,
            }
        }),
    }

    rsx! {
        ElementList { element_list }
    }
}

#[component]
pub fn FilterTypeSelector(mut filter_type_builder_sig: Signal<FilterTypeBuilder>) -> Element {
    rsx! {
        LabeledSelect {
            id: "nodeFilterTypeSelector",
            label: "Filter type definition",
            options: select_options_from_enum_iterator(&*filter_type_builder_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ftb) = FilterTypeBuilder::default_from_name(val.as_str()) {
                    filter_type_builder_sig.set(ftb);
                }
            },
        }
    }
}
