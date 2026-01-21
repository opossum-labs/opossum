use crate::components::node_editor::{
    accordion::ElementList,
    hooks::use_update_signal_with_reactive_prop,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::filter_type_editor::{
        ConstantFilterTypeEditor, SpectralFilterTypeEditor,
    },
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use opossum_core::{
    nodes::SplittingConfigBuilder, prelude::Property, utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn SplitterTypeEditor(
    node_id: Uuid,
    splitting_config_builder: SplittingConfigBuilder,
    property_key: String,
    property: Property,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    use_context_provider(|| property);

    let splitting_config_builder_sig = use_signal(|| splitting_config_builder.clone());
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        splitting_config_builder,
        splitting_config_builder_sig,
        on_change,
    );

    let mut element_list = vec![rsx! {
        SplittingConfigSelector { splitting_config_builder_sig }
    }];

    match &*splitting_config_builder_sig.read() {
        SplittingConfigBuilder::FixedRatio(transmission) => element_list.push(rsx! {
            ConstantFilterTypeEditor {
                transmission: *transmission,
                builder_sig: splitting_config_builder_sig,
            }
        }),
        SplittingConfigBuilder::Spectrum(spectral_filter_builder) => element_list.push(rsx! {
            SpectralFilterTypeEditor {
                spectral_filter_builder: spectral_filter_builder.clone(),
                builder_sig: splitting_config_builder_sig,
            }
        }),
    }
    rsx! {
        ElementList { element_list }
    }
}

#[component]
pub fn SplittingConfigSelector(
    splitting_config_builder_sig: Signal<SplittingConfigBuilder>,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "splitterConfigBuilderPropertySelection",
            label: "Splitting configuration",
            options: select_options_from_enum_iterator(&*splitting_config_builder_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(splitting_config_builder) = SplittingConfigBuilder::default_from_name(
                    val.as_str(),
                ) {
                    splitting_config_builder_sig.set(splitting_config_builder);
                }
            },
        }
    }
}
