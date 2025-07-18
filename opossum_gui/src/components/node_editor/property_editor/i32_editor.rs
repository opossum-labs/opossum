use crate::components::node_editor::{inputs::input_components::LabeledInput, CallbackWrapper};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::Proptype;

#[component]
pub fn I32Editor(int32: i32, property_key: String, prop_type_sig: Signal<Proptype>) -> Element {
    rsx! {
        LabeledInput {
            id: format!("i32Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{int32}"),
            r#type: "number",
            onchange: on_i32_input_change(prop_type_sig),
        }
    }
}

fn on_i32_input_change(mut signal: Signal<Proptype>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<i32>() {
            signal.set(Proptype::I32(val));
        }
    })
}
