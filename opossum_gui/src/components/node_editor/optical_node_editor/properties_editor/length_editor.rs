use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledInput, node_config_editor::NodeChange,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::millimeter;
use uom::si::{f64::Length, length::millimeter};

#[component]
pub fn LengthEditor(
    length: Length,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let length_sig = use_signal(|| length);

    use_set_node_change_property(&property_key, length, length_sig, node_change);

    rsx! {
        LabeledInput {
            id: format!("lengthProperty{property_key}").to_camel_case(),
            label: format!("{} in mm", property_key.to_sentence_case()),
            value: format!("{:.3}", length_sig.read().get::<millimeter>()),
            r#type: "number",
            onchange: on_length_input_change(length_sig),
        }
    }
}

fn on_length_input_change(mut length_sig: Signal<Length>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            length_sig.set(millimeter!(length));
        }
    })
}
