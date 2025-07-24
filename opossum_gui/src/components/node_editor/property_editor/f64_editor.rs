use crate::components::node_editor::{
    inputs::input_components::LabeledInput, node_editor_component::NodeChange,
    property_editor::use_set_node_change_property, CallbackWrapper,
};
use dioxus::prelude::*;
use inflector::Inflector;

#[component]
pub fn F64Editor(
    float64: f64,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let float64_sig = use_signal(|| float64);
    use_set_node_change_property(&property_key, float64, float64_sig, node_change);

    rsx! {
        LabeledInput {
            id: format!("float64Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{:.3}", float64_sig.read()),
            r#type: "number",
            onchange: on_float64_input_change(float64_sig),
        }
    }
}

fn on_float64_input_change(mut signal: Signal<f64>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            signal.set(val);
        }
    })
}
