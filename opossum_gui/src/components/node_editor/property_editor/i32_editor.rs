use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledInput, node_editor_component::NodeChange,
    property_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;

#[component]
pub fn I32Editor(
    int32: i32,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let int32_sig = use_signal(|| int32);
    use_set_node_change_property(&property_key, int32, int32_sig, node_change);

    rsx! {
        LabeledInput {
            id: format!("i32Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{}", int32_sig.read()),
            r#type: "number",
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
