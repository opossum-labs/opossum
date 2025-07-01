use crate::components::node_editor::{accordion::LabeledInput, CallbackWrapper};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{millimeter, Proptype};
use uom::si::{f64::Length, length::millimeter};

#[component]
pub fn LengthEditor(
    length: Length,
    property_key: String,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    rsx! {
        LabeledInput {
            id: format!("lengthProperty{property_key}").to_camel_case(),
            label: format!("{} in mm", property_key.to_sentence_case()),
            value: format!("{}", length.get::<millimeter>()),
            r#type: "number",
            onchange: on_length_input_change(prop_type_sig),
        }
    }
}

fn on_length_input_change(mut signal: Signal<Proptype>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            signal.set(Proptype::Length(millimeter!(length)));
        }
    })
}
