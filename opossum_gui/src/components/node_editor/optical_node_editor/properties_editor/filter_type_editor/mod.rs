mod constant_transmission_editor;
mod spectral_transmission_editor;

use crate::components::node_editor::{
    accordion::ElementList,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
pub use constant_transmission_editor::ConstantFilterTypeEditor;
use dioxus::prelude::*;
use opossum_core::{prelude::FilterTypeBuilder, utils::default_from_name::DefaultFromName};
pub use spectral_transmission_editor::SpectralFilterTypeEditor;
use uuid::Uuid;

#[component]
pub fn FilterTypeEditor(
    node_id: Memo<Uuid>,
    filter_type_builder: FilterTypeBuilder,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let filter_type_builder_sig = use_signal(|| filter_type_builder.clone());
    let on_save = on_save_proptype_handler(
        filter_type_builder_sig,
        property_key,
        on_change,
        node_id.into(),
    );

    let mut element_list: Vec<Result<VNode, RenderError>> = vec![rsx! {
        FilterTypeSelector { filter_type_builder_sig, on_spectral_filter_change: on_save }
    }];

    match &*filter_type_builder_sig.read() {
        FilterTypeBuilder::Constant(transmission) => element_list.push(rsx! {
            ConstantFilterTypeEditor {
                transmission: transmission.transmission().value,
                on_transmission_change: on_save,
                readonly,
            }
        }),
        FilterTypeBuilder::Spectrum(spectral_filter_builder) => element_list.push(rsx! {
            SpectralFilterTypeEditor {
                spectral_filter_builder: spectral_filter_builder.clone(),
                on_spectral_filter_change: on_save,
                readonly,
            }
        }),
    }

    rsx! {
        ElementList { element_list }
    }
}

#[component]
pub fn FilterTypeSelector(
    filter_type_builder_sig: ReadSignal<FilterTypeBuilder>,
    on_spectral_filter_change: EventHandler<FilterTypeBuilder>,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "nodeFilterTypeSelector",
            label: "Filter type definition",
            options: select_options_from_enum_iterator(&*filter_type_builder_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ftb) = FilterTypeBuilder::default_from_name(val.as_str()) {
                    on_spectral_filter_change.call(ftb);
                }
            },
        }
    }
}
