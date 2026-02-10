use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop, inputs::input_components::LabeledCheckboxInput,
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use uuid::Uuid;

#[component]
pub fn BoolEditor(
    node_id: Memo<Uuid>,
    b: bool,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let bool_memo = use_memo(use_reactive!(|b| b));

    rsx! {
        LabeledCheckboxInput {
            id: format!("boolProperty{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{}", *bool_memo.read()),
            onchange: move |e: Event<FormData>| {
                if let Ok(val) = e.data.value().parse::<bool>() {
                    on_change
                        .call(NodeChangeEvent {
                            node_id: *node_id.read(),
                            action: NodeChangeAction::Property(property_key.clone(), val.into()),
                        });
                }
            },
        }
    }
}
