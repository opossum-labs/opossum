use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop, inputs::input_components::NodeConfigUnitInput,
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::meter;
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn LengthEditor(
    node_id: Uuid,
    length: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut length_sig = use_signal(|| length);
    use_update_signal_with_reactive_prop(length, length_sig);
    let value_memo = use_memo(move || length_sig.read().value);

    rsx! {
        NodeConfigUnitInput {
            id: format!("lengthProperty{property_key}").to_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: value_memo,
            base_unit: "m",
            onchange: move |new_length: f64| {
                if relative_ne!(length.value, new_length) {
                    length_sig.set(meter!(new_length));
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
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
