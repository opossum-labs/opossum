use crate::components::node_editor::{
    accordion::ElementList,
    hooks::use_synced_signal,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    optical_node_editor::properties_editor::filter_type_editor::{
        ConstantFilterTypeEditor, SpectralFilterTypeEditor,
    },
};
use dioxus::prelude::*;
use opossum_core::{coatings::SplittingConfigBuilder, utils::default_from_name::DefaultFromName};

/// Edits a splitting configuration: a fixed transmission or a transmission spectrum. Every change
/// is passed to `on_change`.
#[component]
pub fn SplittingConfigEditor(
    splitting_config_builder: SplittingConfigBuilder,
    on_change: EventHandler<SplittingConfigBuilder>,
    readonly: bool,
) -> Element {
    let splitting_config_builder_sig = use_synced_signal(splitting_config_builder);

    let mut element_list = vec![rsx! {
        SplittingConfigSelector { splitting_config_builder_handler: on_change, splitting_config_builder_sig, readonly }
    }];

    match &*splitting_config_builder_sig.read() {
        SplittingConfigBuilder::FixedRatio(transmission) => {
            element_list.push(rsx! {
                ConstantFilterTypeEditor {
                    transmission: *transmission,
                    on_transmission_change: on_change,
                    readonly,
                }
            });
        }
        SplittingConfigBuilder::Spectrum(spectral_filter_builder) => element_list.push(rsx! {
            SpectralFilterTypeEditor {
                spectral_filter_builder: spectral_filter_builder.clone(),
                on_spectral_filter_change: on_change,
                readonly,
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
    readonly: bool,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "splitterConfigBuilderPropertySelection",
            label: "Splitting configuration",
            options: select_options_from_enum_iterator(&*splitting_config_builder_sig.read(), None),
            readonly,
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
