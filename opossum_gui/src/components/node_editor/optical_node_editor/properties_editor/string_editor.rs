use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop, inputs::input_components::{FlushableTextInput, LabeledInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid;

#[component]
pub fn StringEditor(
    node_id: Memo<Uuid>,
    s: String,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let string_memo = use_memo(use_reactive!(|s| s.clone()));
    rsx! {
        FlushableTextInput {
            id: format!("stringProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: string_memo(),
            on_save: move |new_val: String| {
                if string_memo() != new_val {
                    on_change
                        .call(NodeChangeEvent {
                            node_id: *node_id.read(),
                            action: NodeChangeAction::Property(
                                property_key.clone(),
                                new_val.into(),
                            ),
                        });
                }
            },
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
        }
    }
}
