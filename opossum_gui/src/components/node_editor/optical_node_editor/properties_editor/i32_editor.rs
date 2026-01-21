use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop, inputs::input_components::LabeledInput,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid;

#[component]
pub fn I32Editor(
    node_id: Uuid,
    int32: i32,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut int32_sig = use_signal(|| int32);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        int32,
        int32_sig,
        on_change,
    );

    rsx! {
        LabeledInput {
            id: format!("i32Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{}", *int32_sig.read()),
            r#type: "number",
            step: Some("1"),
            onchange: move |e: Event<FormData>| {
                if let Ok(val) = e.data.value().parse::<i32>() {
                    int32_sig.set(val);
                }
            },
        }
    }
}
