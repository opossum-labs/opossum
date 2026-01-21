use crate::components::node_editor::{
    inputs::input_components::LabeledInput,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::{
        use_set_node_change_property, use_update_signal_with_reactive_prop,
    },
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid;

#[component]
pub fn StringEditor(
    node_id: Uuid,
    s: String,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut string_sig = use_signal(|| s.clone());
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        s,
        string_sig,
        on_change,
    );

    rsx! {
        LabeledInput {
            id: format!("stringProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: string_sig,
            r#type: "text",
            onchange: move |e: Event<FormData>| { string_sig.set(e.data.value()) },
        }
    }
}
