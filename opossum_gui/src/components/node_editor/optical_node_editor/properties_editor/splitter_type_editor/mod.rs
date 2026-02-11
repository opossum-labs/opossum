use crate::components::node_editor::{
    accordion::ElementList,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::{
        filter_type_editor::{ConstantFilterTypeEditor, SpectralFilterTypeEditor},
        on_save_proptype_handler,
    },
};
use dioxus::prelude::*;
use opossum_core::{nodes::SplittingConfigBuilder, utils::default_from_name::DefaultFromName};
use uuid::Uuid;

#[component]
pub fn SplitterTypeEditor(
    node_id: Memo<Uuid>,
    splitting_config_builder: SplittingConfigBuilder,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let splitting_config_builder_sig = use_signal(|| splitting_config_builder.clone());
    let on_save = on_save_proptype_handler(
        splitting_config_builder_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    let mut element_list = vec![rsx! {
        SplittingConfigSelector { splitting_config_builder_handler: on_save, splitting_config_builder_sig }
    }];

    match &*splitting_config_builder_sig.read() {
        SplittingConfigBuilder::FixedRatio(transmission) => {
            element_list.push(rsx! {
                ConstantFilterTypeEditor {
                    transmission: *transmission,
                    on_transmission_change: on_save,
                }
            });
        }
        SplittingConfigBuilder::Spectrum(spectral_filter_builder) => element_list.push(rsx! {
            SpectralFilterTypeEditor {
                spectral_filter_builder: spectral_filter_builder.clone(),
                on_spectral_filter_change: on_save,
            }
        }),
    }
    rsx! {
        ElementList { element_list }
    }
}

#[component]
pub fn SplittingConfigSelector(
    splitting_config_builder_handler: EventHandler<SplittingConfigBuilder>,
    splitting_config_builder_sig: ReadSignal<SplittingConfigBuilder>,
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
                    splitting_config_builder_handler.call(splitting_config_builder);
                }
            },
        }
    }
}
