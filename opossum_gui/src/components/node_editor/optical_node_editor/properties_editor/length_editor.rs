use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop,
    inputs::input_components::NodeConfigUnitInput,
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::meter;
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn LengthEditor(
    node_id: Memo<Uuid>,
    length: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let length_memo = use_memo(use_reactive!(|length| length.value));
    rsx! {
        NodeConfigUnitInput {
            id: format!("lengthProperty{property_key}").to_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: length_memo(),
            base_unit: "m",
            onchange: move |new_length: f64| {
                if relative_ne!(length_memo(), new_length) {
                    on_change
                        .call(NodeChangeEvent {
                            node_id: *node_id.read(),
                            action: NodeChangeAction::Property(
                                property_key.clone(),
                                meter!(new_length).into(),
                            ),
                        });
                }
            },
        }
    }
}
