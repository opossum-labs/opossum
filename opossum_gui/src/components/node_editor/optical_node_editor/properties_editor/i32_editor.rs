use crate::components::node_editor::{
    hooks::use_synced_signal, inputs::input_components::FlushableTextInput,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid;

#[component]
pub fn I32Editor(
    node_id: Memo<Uuid>,
    int32: i32,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let int32_sig = use_synced_signal(int32);
    let on_save =
        on_save_proptype_handler(int32_sig, property_key.clone(), on_change, node_id.into());

    rsx! {
        FlushableTextInput {
            id: format!("i32Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: int32_sig().to_string(),
            on_save: move |new_val: String| {
                if let Ok(val) = new_val.parse::<i32>() {
                    on_save.call(val);
                }
            },
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
            readonly,
        }
    }
}
