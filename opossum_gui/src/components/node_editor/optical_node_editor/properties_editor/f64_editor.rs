use crate::components::node_editor::{
    inputs::input_components::FlushableTextInput, node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::prelude::Property;
use uuid::Uuid;

#[component]
pub fn F64Editor(
    node_id: Memo<Uuid>,
    float64: f64,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let float64_sig = use_signal(|| float64);
    let on_save =
        on_save_proptype_handler(float64_sig, property_key.clone(), on_change, node_id.into());

    rsx! {
        FlushableTextInput {
            id: format!("float64Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: float64_sig().to_string(),
            on_save: move |new_val: String| {
                if let Ok(val) = new_val.parse::<f64>() {
                    on_save.call(val);
                }
            },
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
        }
    }
}
