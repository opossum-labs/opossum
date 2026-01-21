use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop, inputs::input_components::LabeledCheckboxInput,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid;

#[component]
pub fn BoolEditor(
    node_id: Uuid,
    b: bool,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut bool_sig = use_signal(|| b);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(*bound_node_id.read(), &property_key, b, bool_sig, on_change);

    rsx! {
        LabeledCheckboxInput {
            id: format!("boolProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{}", *bool_sig.read()),
            onchange: move |e: Event<FormData>| {
                if let Ok(val) = e.data.value().parse::<bool>() {
                    bool_sig.set(val);
                }
            },
        }
    }
}
