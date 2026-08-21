use crate::components::node_editor::{
    hooks::use_synced_signal, inputs::input_components::FlushableTextInput,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid;

#[component]
pub fn StringEditor(
    node_id: ReadSignal<Uuid>,
    s: String,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let string_sig = use_synced_signal(s);
    let on_save = on_save_proptype_handler(string_sig, property_key.clone(), on_change, node_id);

    rsx! {
        FlushableTextInput {
            id: format!("stringProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: string_sig(),
            on_save,
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
            readonly,
        }
    }
}
