use crate::components::node_editor::{
    inputs::input_components::NodeConfigPlainF64Input, node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
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
        NodeConfigPlainF64Input {
            id: format!("float64Property{property_key}").to_camel_case(),
            label: property_key.to_sentence_case(),
            value: float64_sig,
            onchange: move |new_val: f64| {
                if relative_ne!(* float64_sig.read(), new_val, epsilon = 0.0) {
                    on_save.call(new_val);
                }
            },
        }
    }
}
