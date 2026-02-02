use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop, inputs::input_components::{LabeledInput, NodeConfigUnitInput},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{meter, millimeter};
use uom::si::{f64::Length, length::millimeter};
use uuid::Uuid;

#[component]
pub fn LengthEditor(
    node_id: Uuid,
    length: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut length_sig = use_signal(|| length);
    let value_signal = use_signal(|| length_sig.read().value);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        length,
        length_sig,
        on_change,
    );

    rsx! {
        NodeConfigUnitInput {
            id: format!("lengthProperty{property_key}").to_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: value_signal,
            base_unit: "m",
            onchange: move |length: f64| length_sig.set(meter!(length)),
        }
        // LabeledInput {
        //     id: format!("lengthProperty{property_key}").to_camel_case(),
        //     label: format!("{} in mm", property_key.to_sentence_case()),
        //     value: format!("{:.3}", length_sig.read().get::<millimeter>()),
        //     r#type: "number",
        //     onchange: move |e: Event<FormData>| {
        //         if let Ok(length) = e.data.value().parse::<f64>() {
        //             length_sig.set(millimeter!(length));
        //         }
        //     },
        // }
    }
}
