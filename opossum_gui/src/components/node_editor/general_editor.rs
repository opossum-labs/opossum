use crate::components::node_editor::{
    accordion::{AccordionItem, LabeledInput},
    node_editor_component::NodeChange,
};
use dioxus::prelude::*;
use opossum_backend::{Fluence, J_per_cm2};
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uuid::Uuid;

#[component]
pub fn GeneralEditor(
    node_id: Uuid,
    node_type: String,
    node_name: String,
    node_lidt: Fluence,
) -> Element {
    let accordion_content = vec![rsx! {
            NodeIDInput {node_id}
            NodeTypeInput {node_type}
            NodeNameInput{node_name}
            NodeLIDTInput {node_lidt},
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
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    rsx! {
        LabeledInput {
            id: "inputNodeName",
            label: "Node Name",
            value: node_name,
            onchange: Some(name_onchange(node_change_signal)),
        }
    }
}

pub fn name_onchange(mut signal: Signal<Option<NodeChange>>) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
        let Ok(name) = e.data.value().parse::<String>();
        signal.set(Some(NodeChange::Name(name)));
    })
}

#[component]
pub fn NodeLIDTInput(node_lidt: Fluence) -> Element {
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    rsx! {
        LabeledInput {
            id: "inputNodeLIDT",
            label: "LIDT in J/cm²",
            value: format!("{:.2}", node_lidt.get::<joule_per_square_centimeter>()),
            onchange: Some(lidt_onchange(node_change_signal)),
            r#type: "number",
        }
    }
}

pub fn lidt_onchange(mut signal: Signal<Option<NodeChange>>) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
        if let Ok(lidt) = e.data.parsed::<f64>() {
            signal.set(Some(NodeChange::LIDT(J_per_cm2!(lidt))));
        }
    })
}

#[component]
pub fn NodeIDInput(node_id: Uuid) -> Element {
    rsx! {
        LabeledInput {
            id: "inputNodeID",
            label: "Node ID",
            value: format!("{node_id}"),
            readonly: true,
        }
    }
}

#[component]
pub fn NodeTypeInput(node_type: String) -> Element {
    rsx! {
        LabeledInput {
            id: "inputNodeType",
            label: "Node Type",
            value: node_type,
            readonly: true,
        }
    }
}
