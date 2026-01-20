use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledCheckboxInput,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid; // Import hinzugefügt

#[component]
pub fn BoolEditor(
    node_id: Uuid, // Prop hinzugefügt
    b: bool, 
    property_key: String
) -> Element {
    let bool_sig = use_signal(|| b);
    
    // node_id an den Hook übergeben
    use_set_node_change_property(node_id, &property_key, b, bool_sig);

    rsx! {
        LabeledCheckboxInput {
            id: format!("boolProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{}", b),
            onchange: on_bool_input_change(bool_sig),
        }
    }
}

fn on_bool_input_change(mut signal: Signal<bool>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<bool>() {
            signal.set(val);
        }
    })
}