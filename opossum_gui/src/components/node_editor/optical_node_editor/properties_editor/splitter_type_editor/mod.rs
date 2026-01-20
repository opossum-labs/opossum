use crate::components::node_editor::{
    accordion::ElementList,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    optical_node_editor::properties_editor::{
        filter_type_editor::{ConstantFilterTypeEditor, SpectralFilterTypeEditor},
        use_set_node_change_property,
    },
};
use dioxus::prelude::*;
use opossum_core::{
    nodes::SplittingConfigBuilder, prelude::Property, utils::default_from_name::DefaultFromName,
};
use uuid::Uuid; // Import hinzugefügt

#[component]
pub fn SplitterTypeEditor(
    node_id: Uuid, // Prop hinzugefügt
    splitting_config_builder: SplittingConfigBuilder,
    property_key: String,
    property: Property,
) -> Element {
    use_context_provider(|| property);

    let splitting_config_builder_sig = use_signal(|| splitting_config_builder.clone());
    
    // node_id an den Hook übergeben
    use_set_node_change_property(
        node_id,
        &property_key,
        splitting_config_builder,
        splitting_config_builder_sig,
    );
    
    let mut element_list = vec![rsx! {
    SplittingConfigSelector {splitting_config_builder_sig}}];

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