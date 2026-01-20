#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::{
    CallbackWrapper, // Wichtig: Wird für den Wrapper benötigt
    accordion::AccordionItem,
    inputs::input_components::{LabeledCheckboxInput, LabeledInput},
    node_config_editor::NodeChangeAction,
    optical_node_editor::properties_editor::use_update_signal_with_reactive_prop,
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
) -> Element {
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();

    // --- LAGGING ID PATTERN ---
    let mut bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);

    let accordion_content = vec![
        rsx! {
            NodeTypeInput { node_type, label: "Node Type" }
        },
        rsx! {
            NodeNameInput {
                value: name,
                on_valid_change: move |new_name: String| {
                    node_config_processor.send(NodeChangeAction::Name(*bound_node_id.peek(), new_name));
                }
            }
        },
        rsx! {
            NodeLIDTInput {
                value: lidt,
                on_valid_change: move |new_fluence: Fluence| {
                    node_config_processor.send(NodeChangeAction::Lidt(*bound_node_id.peek(), new_fluence));
                }
            }
        },
        rsx! {
            NodeInvertedInput {
                value: inverted,
                label: "Invert Node",
                on_valid_change: move |new_state: bool| {
                    node_config_processor.send(NodeChangeAction::Inverted(*bound_node_id.peek(), new_state));
                }
            }
        }
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
pub fn NodeNameInput(
    value: String,
    on_valid_change: EventHandler<String>,
) -> Element {
    let mut text_state = use_signal(|| value.clone());

    use_effect(use_reactive!(|value| {
        text_state.set(value);
    }));

    rsx! {
        LabeledInput {
            id: "inputNodeName",
            label: "Node Name",
            value: text_state,
            // FIX: Closure in CallbackWrapper verpackt
            onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                let new_val = e.data.value();
                text_state.set(new_val.clone());
                on_valid_change.call(new_val);
            }),
        }
    }
}

#[component]
pub fn NodeLIDTInput(
    value: Fluence,
    on_valid_change: EventHandler<Fluence>,
) -> Element {
    let mut text_state = use_signal(|| format!("{:.2}", value.get::<joule_per_square_centimeter>()));

    use_effect(use_reactive!(|value| {
        text_state.set(format!("{:.2}", value.get::<joule_per_square_centimeter>()));
    }));

    rsx! {
        LabeledInput {
            id: "inputNodeLIDT",
            label: "LIDT in J/cm²",
            value: text_state,
            r#type: "number",
            min: Some("0.0"),
            // FIX: Closure in CallbackWrapper verpackt
            onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                let input_str = e.data.value();

                if let Ok(parsed_num) = input_str.parse::<f64>() {
                    if parsed_num >= 0.0 {
                        on_valid_change.call(J_per_cm2!(parsed_num));
                        return;
                    }
                }

                text_state.set(format!("{:.2}", value.get::<joule_per_square_centimeter>()));
            }),
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
            id: "inputNodeInverted",
            label,
            value,
            // FIX: Closure in CallbackWrapper verpackt
            onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                if let Ok(new_val) = e.data.parsed::<bool>() {
                    on_valid_change.call(new_val);
                }
            }),
        }
    }
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