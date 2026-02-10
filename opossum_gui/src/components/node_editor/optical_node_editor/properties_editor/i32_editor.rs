use crate::components::node_editor::{inputs::input_components::FlushableTextInput,
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
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
) -> Element {
    let int32_memo = use_memo(use_reactive!(|int32| int32));
    rsx! {
        FlushableTextInput {
            id: format!("i32Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: int32_memo().to_string(),
            on_save: move |new_val: String| {
                if let Ok(val) = new_val.parse::<i32>() {
                    if int32_memo() != val {
                        on_change
                            .call(NodeChangeEvent {
                                node_id: *node_id.read(),
                                action: NodeChangeAction::Property(
                                    property_key.clone(),
                                    val.into(),
                                ),
                            });
                    }
                }
            },
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
        }
    }
}
