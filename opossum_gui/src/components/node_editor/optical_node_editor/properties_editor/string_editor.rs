use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledInput,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid; // Import hinzugefügt

#[component]
pub fn StringEditor(
    node_id: Uuid, // Prop hinzugefügt
    s: String, 
    property_key: String
) -> Element {
    let string_sig = use_signal(|| s.clone());
    
    // node_id an den Hook übergeben
    use_set_node_change_property(node_id, &property_key, s.clone(), string_sig);

    rsx! {
        LabeledInput {
            id: format!("stringProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: s,
            r#type: "text",
            onchange: on_string_input_change(string_sig),
        }
    }
}

fn on_string_input_change(mut signal: Signal<String>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        signal.set(e.data.value());
    })
}