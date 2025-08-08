use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledCheckboxInput,
    node_config_editor::NodeChange,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;

#[component]
pub fn BoolEditor(
    b: bool,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let bool_sig = use_signal(|| b);
    use_set_node_change_property(&property_key, b, bool_sig, node_change);

    rsx! {
        LabeledCheckboxInput {
            id: format!("boolProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{}", bool_sig.read()),
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
