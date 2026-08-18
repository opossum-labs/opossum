use opossum_core::refractive_index::RefractiveIndexType;

use dioxus::prelude::*;
use inflector::Inflector;

use crate::components::{
    inputs::refractive_index_editor::RefractiveIndexEditor,
    node_editor::{
        hooks::use_synced_signal, node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::on_save_proptype_handler,
    },
};
use uuid::Uuid;

#[component]
pub fn RefractiveIndexPropertyEditor(
    /// id of the node element
    node_id: ReadSignal<Uuid>,
    ref_ind_type: RefractiveIndexType,
    /// name of the property
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    /// If true, disables all input fields and dropdowns.
    #[props(default = false)]
    readonly: bool,
) -> Element {
    // Synchronize the local state with external changes
    let ref_ind_type_sig = use_synced_signal(ref_ind_type);

    // Create the OPOSSUM specific save handler
    let on_save =
        on_save_proptype_handler(ref_ind_type_sig, property_key.clone(), on_change, node_id);

    // Generate a unique base ID for HTML elements (matches previous implementation)
    let dynamic_base_id = format!("refractiveIndexProperty{property_key}").to_camel_case();

    rsx! {
        // Instantiate the decoupled, generic editor
        RefractiveIndexEditor {
            value: ref_ind_type_sig,
            on_change: move |new_type: RefractiveIndexType| {
                on_save.call(new_type);
            },
            base_id: dynamic_base_id,
            readonly,
        }
    }
}
