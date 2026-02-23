#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::{node_editor::{
    accordion::AccordionItem,
    inputs::input_components::{
        FlushableTextInput, LabeledCheckboxInput, LabeledInput, NodeConfigUnitInput,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::UINodeAttr,
}, scenery_editor::GraphState};
use dioxus::prelude::*;
use opossum_core::J_per_cm2;
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uuid::Uuid;

#[component]
pub fn GeneralEditor(
    node_attr: ReadSignal<UINodeAttr>,
    node_id: Memo<Uuid>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let graph_state = use_context::<GraphState>();
    let accordion_content = if node_attr.read().node_id == *node_id.read() {
        let node_id = node_attr.read().node_id;
        let node_type = node_attr.read().node_type.clone();
        let name = node_attr.read().name.clone();
        let lidt = node_attr.read().lidt;
        let inverted = node_attr.read().inverted;
        vec![
            rsx! {
                NodeTypeInput { node_type: node_type, label: "Node Type" }
            },
            rsx! {
                FlushableTextInput {
                    id: format!("nodeName_{}", node_id),
                    label: "Node Name".to_string(),
                    value: name,
                    container_class: "form-floating border-start".to_string(),
                    input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
                    label_class: "form-label text-secondary".to_string(),
                    on_save: move |new_val: String| {
                        on_change.call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::Name{name: new_val, graph_id: graph_state.id},
                        });
                    },
                }
            },
            rsx! {
                NodeConfigUnitInput {
                        id: format!("nodeLidt_{}", node_id),
                        label: "Damage Threshold".to_string(),
                        value: lidt.get::<joule_per_square_centimeter>(),
                        base_unit: "J/cm²",
                        onchange: move |new_lidt: f64| {
                        if new_lidt >= 0.0 {
                            // lidt_sig.set(new_lidt);
                                on_change.call(NodeChangeEvent {
                                    node_id,
                                    action: NodeChangeAction::Lidt(J_per_cm2!(new_lidt)),
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
                            action: NodeChangeAction::Inverted{inverted: new_state, graph_id: graph_state.id},
                        });
                    }
                }
            },
        ]
    } else {
        vec![]
    };

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
