#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::{
    CallbackWrapper,
    accordion::AccordionItem,
    inputs::input_components::{LabeledCheckboxInput, LabeledInput},
    node_config_editor::NodeChangeAction,
};
use dioxus::prelude::*;
use opossum_core::{J_per_cm2, nodes::fluence_detector::Fluence};
use uom::si::radiant_exposure::joule_per_square_centimeter;

#[component]
pub fn GeneralEditor(
    node_type: String,
    node_name: String,
    node_lidt: Fluence,
    node_inverted: bool,
) -> Element {
    let accordion_content = vec![rsx! {
            NodeTypeInput {node_type, label: "Node Type"},
            NodeNameInput {node_name},
            NodeLIDTInput {node_lidt},
            NodeInvertedInput {node_inverted, label: "Invert Node"},
    }];
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
pub fn NodeNameInput(node_name: String) -> Element {
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();
    rsx! {
        LabeledInput {
            id: "inputNodeName",
            label: "Node Name",
            value: node_name,
            onchange: name_onchange(node_config_processor),
        }
    }
}

#[must_use]
pub fn name_onchange(node_config_processor: Coroutine<NodeChangeAction>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        let Ok(name) = e.data.value().parse::<String>();
        node_config_processor.send(NodeChangeAction::Name(name));
    })
}

#[component]
pub fn NodeLIDTInput(node_lidt: Fluence) -> Element {
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();
    rsx! {
        LabeledInput {
            id: "inputNodeLIDT",
            label: "LIDT in J/cm²",
            value: format!("{:.2}", node_lidt.get::<joule_per_square_centimeter>()),
            onchange: lidt_onchange(node_config_processor),
            r#type: "number",
        }
    }
}

#[must_use]
pub fn lidt_onchange(node_config_processor: Coroutine<NodeChangeAction>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(lidt) = e.data.parsed::<f64>() {
            node_config_processor.send(NodeChangeAction::Lidt(J_per_cm2!(lidt)));
        }
    })
}

#[component]
pub fn NodeTypeInput(node_type: String, label: &'static str) -> Element {
    rsx! {
        LabeledInput {
            id: "inputNodeType",
            label,
            value: node_type,
            readonly: true,
            onchange: CallbackWrapper::noop(),
        }
    }
}

#[component]
pub fn NodeInvertedInput(node_inverted: bool, label: &'static str) -> Element {
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();
    rsx! {
        LabeledCheckboxInput {
            id: "inputNodeInverted",
            label,
            value: node_inverted,
            onchange: inverted_onchange(node_config_processor),
        }
    }
}

#[must_use]
pub fn inverted_onchange(node_config_processor: Coroutine<NodeChangeAction>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(inverted) = e.data.parsed::<bool>() {
            node_config_processor.send(NodeChangeAction::Inverted(inverted));
        }
    })
}
