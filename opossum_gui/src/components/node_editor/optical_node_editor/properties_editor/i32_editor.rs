use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledInput, node_config_editor::NodeChangeAction,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;

#[component]
pub fn I32Editor(int32: i32, property_key: String) -> Element {
    let int32_sig = use_signal(|| int32);
    use_set_node_change_property(&property_key, int32, int32_sig);

    rsx! {
        LabeledInput {
            id: format!("i32Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{}", int32),
            r#type: "number",
            step: Some("1"),
            onchange: on_i32_input_change(int32_sig),
        }
    }
}

fn on_i32_input_change(mut signal: Signal<i32>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<i32>() {
            signal.set(val);
        }
    })
}
