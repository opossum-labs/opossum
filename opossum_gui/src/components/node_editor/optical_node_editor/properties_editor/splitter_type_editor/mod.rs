use crate::components::node_editor::{
    hooks::use_synced_signal,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::{
        on_save_proptype_handler, surface_config_editor::coating_editor::SplittingConfigEditor,
    },
};
use dioxus::prelude::*;
use opossum_core::coatings::SplittingConfigBuilder;
use uuid::Uuid;

/// Edits the splitting configuration of a beam splitter, stored in the property `property_key`.
#[component]
pub fn SplitterTypeEditor(
    node_id: ReadSignal<Uuid>,
    splitting_config_builder: SplittingConfigBuilder,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let splitting_config_builder_sig = use_synced_signal(splitting_config_builder);
    let on_save = on_save_proptype_handler(
        splitting_config_builder_sig,
        property_key,
        on_change,
        node_id,
    );
    rsx! {
        SplittingConfigEditor {
            splitting_config_builder: splitting_config_builder_sig.read().clone(),
            on_change: on_save,
            readonly,
        }
    }
}
