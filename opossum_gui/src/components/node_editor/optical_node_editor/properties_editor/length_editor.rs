use crate::components::node_editor::{
    inputs::input_components::NodeConfigUnitInput, node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::meter;
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn LengthEditor(
    node_id: Memo<Uuid>,
    length: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let length_sig = use_signal(|| length);
    let on_save =
        on_save_proptype_handler(length_sig, property_key.clone(), on_change, node_id.into());

    rsx! {
        NodeConfigUnitInput {
            id: format!("lengthProperty{property_key}").to_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: length_sig.read().value,
            base_unit: "m",
            onchange: move |new_length: f64| {
                if relative_ne!(length_sig.read().value, new_length, epsilon = 0.0) {
                    on_save.call(meter!(new_length));
                }
            },
        }
    }
}
