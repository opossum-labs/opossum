use crate::components::node_editor::{inputs::input_components::LabeledInput, CallbackWrapper};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::Proptype;

#[component]
pub fn F64Editor(float64: f64, property_key: String, prop_type_sig: Signal<Proptype>) -> Element {
    rsx! {
        LabeledInput {
            id: format!("float64Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{float64}"),
            r#type: "number",
            onchange: on_float64_input_change(prop_type_sig),
        }
    }
}

fn on_float64_input_change(mut signal: Signal<Proptype>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            signal.set(Proptype::F64(val));
        }
    })
}
