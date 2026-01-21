#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::{
    accordion::AccordionItem,
    inputs::input_components::{FlushableTextInput, LabeledCheckboxInput, LabeledInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use dioxus::prelude::*;
use opossum_core::{J_per_cm2, nodes::fluence_detector::Fluence};
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uuid::Uuid;

#[component]
pub fn GeneralEditor(
    node_id: Uuid,
    node_type: String,
    name: String,
    lidt: Fluence,
    inverted: bool,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let accordion_content = vec![
        rsx! {
            NodeTypeInput { node_type: node_type, label: "Node Type" }
        },
        rsx! {
            FlushableTextInput {
                id: format!("nodeName_{}", node_id),
                label: "Node Name".to_string(),
                value: name,
                on_save: move |new_val: String| {
                    on_change.call(NodeChangeEvent {
                        node_id,
                        action: NodeChangeAction::Name(new_val),
                    });
                }
            }
        },
        rsx! {
            FlushableTextInput {
                id: format!("nodeLidt_{}", node_id),
                label: "LIDT in J/cm²".to_string(),
                value: format!("{:.2}", lidt.get::<joule_per_square_centimeter>()),
                r#type: "number",
                step: "0.1",
                min: "0.0",
                on_save: move |new_val_str: String| {
                    if let Ok(parsed_num) = new_val_str.parse::<f64>()
                         && parsed_num >= 0.0 {
                             on_change.call(NodeChangeEvent {
                                node_id,
                                action: NodeChangeAction::Lidt(J_per_cm2!(parsed_num)),
                            });
                        }
                }
            }
        },
        rsx! {
            NodeInvertedInput {
                value: inverted,
                label: "Invert Node",
                on_valid_change: move |new_state: bool| {
                    on_change.call(NodeChangeEvent {
                        node_id,
                        action: NodeChangeAction::Inverted(new_state),
                    });
                }
            }
        },
    ];

    rsx! {
        AccordionItem {
            elements: accordion_content,
            header: "General",
            header_id: "generalHeading",
            parent_id: "accordionNodeConfig",
            content_id: "generalCollapse",
        }
    }
}

#[component]
pub fn NodeInvertedInput(
    value: bool,
    label: String,
    on_valid_change: EventHandler<bool>,
) -> Element {
    rsx! {
        LabeledCheckboxInput {
            id: "inputNodeInverted".to_string(),
            label,
            value: format!("{}", value),
            onchange: move |e: Event<FormData>| {
                if let Ok(new_val) = e.data.parsed::<bool>() {
                    on_valid_change.call(new_val);
                }
            },
        }
    }
}

#[component]
pub fn NodeTypeInput(node_type: String, label: &'static str) -> Element {
    rsx! {
        LabeledInput {
            id: "inputNodeType".to_string(),
            label: label.to_string(),
            value: node_type,
            readonly: true,
            onchange: |_| {},
        }
    }
}
