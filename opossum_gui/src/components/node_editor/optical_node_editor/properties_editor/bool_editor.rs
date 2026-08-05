use crate::components::node_editor::{
    hooks::use_synced_signal, inputs::input_components::LabeledCheckboxInput,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid;

#[component]
pub fn BoolEditor(
    node_id: Memo<Uuid>,
    b: bool,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let bool_sig = use_synced_signal(b);
    let on_save =
        on_save_proptype_handler(bool_sig, property_key.clone(), on_change, node_id.into());

    rsx! {
        LabeledCheckboxInput {
            id: format!("boolProperty{property_key}").to_camel_case(),
            label: property_key.to_sentence_case(),
            value: format!("{}", *bool_sig.read()),
            readonly,
            onchange: move |e: Event<FormData>| {
                if let Ok(val) = e.data.value().parse::<bool>() {
                    on_save.call(val);
                }
            },
        }
    }
}
