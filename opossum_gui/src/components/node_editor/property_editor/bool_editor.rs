use crate::components::node_editor::{
    inputs::input_components::LabeledCheckboxInput, CallbackWrapper,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::Proptype;

#[component]
pub fn BoolEditor(b: bool, property_key: String, prop_type_sig: Signal<Proptype>) -> Element {
    rsx! {
        LabeledCheckboxInput {
            id: format!("boolProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{b}"),
            onchange: on_bool_input_change(prop_type_sig),
        }
    }
}

fn on_bool_input_change(mut signal: Signal<Proptype>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<bool>() {
            signal.set(Proptype::Bool(val));
        }
    })
}
