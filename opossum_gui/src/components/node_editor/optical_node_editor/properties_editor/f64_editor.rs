use crate::{
    components::node_editor::{inputs::input_components::FlushableTextInput,
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
    },
};
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::prelude::Property;
use uuid::Uuid;

#[component]
pub fn F64Editor(
    node_id: Memo<Uuid>,
    float64: f64,
    property_key: String,
    property: Property,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let float64_memo = use_memo(use_reactive!(|float64| float64));
   
    rsx! {
        FlushableTextInput {
            id: format!("float64Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: float64_memo().to_string(),
            on_save: move |new_val: String| {
                if let Ok(val) = new_val.parse::<f64>() {
                    if relative_ne!(float64_memo(), val) {
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
