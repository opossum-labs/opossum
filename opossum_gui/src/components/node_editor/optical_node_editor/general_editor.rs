#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::{
    accordion::{AccordionItem, content_id_for_panel},
    inputs::input_components::{FlushableTextInput, LabeledCheckboxInput, LabeledInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use dioxus::prelude::*;
use opossum_core::types::api_types::NodeEditorPanel;
use uuid::Uuid;

#[component]
pub fn GeneralEditor(
    node_id: Uuid,
    name: String,
    node_type: String,
    inverted: bool,
    is_active: bool,
    graph_id: Uuid,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    info!("🔄 Render: GeneralEditor");

    // Stable callback for renaming nodes.
    // Since node_id is passed by value and constant for this component lifecycle,
    // it is safely captured by the move closure.
    let on_name_save = use_callback(move |new_name: String| {
        on_change.call(NodeChangeEvent {
            node_id,
            action: NodeChangeAction::Name(new_name),
        });
    });

    // Stable callback for toggling node inversion.
    // Safely captures the value-based node_id and graph_id.
    let on_inverted_change = use_callback(move |new_state: bool| {
        on_change.call(NodeChangeEvent {
            node_id,
            action: NodeChangeAction::Inverted {
                inverted: new_state,
                graph_id,
            },
        });
    });

    let accordion_content = if is_active {
        vec![
            rsx! {
                NodeTypeInput {
                    node_type,
                    label: "Node Type",
                }
            },
            rsx! {
                FlushableTextInput {
                    id: format!("nodeName_{node_id}"),
                    label: "Node Name".to_string(),
                    value: name,
                    container_class: "form-floating border-start".to_string(),
                    input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
                    label_class: "form-label text-secondary".to_string(),
                    readonly,
                    on_save: on_name_save,
                }
            },
            rsx! {
                NodeInvertedInput {
                    value: inverted,
                    label: "Invert Node",
                    on_valid_change: on_inverted_change,
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
            content_id: content_id_for_panel(NodeEditorPanel::General),
            level: 1,
        }
    }
}

#[component]
pub fn NodeInvertedInput(
    value: bool,
    label: &'static str,
    on_valid_change: EventHandler<bool>,
) -> Element {
    rsx! {
        LabeledCheckboxInput {
            id: "inputNodeInverted".to_string(),
            label: label.to_string(),
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
