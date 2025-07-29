use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledInput, node_editor_component::NodeChange,
    property_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;

#[component]
pub fn StringEditor(
    s: String,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let string_sig = use_signal(|| s.clone());
    use_set_node_change_property(&property_key, s, string_sig, node_change);

    rsx! {
        LabeledInput {
            id: format!("stringProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: string_sig.read().clone(),
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
