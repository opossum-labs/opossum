#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::analyzer_node_editor::AnalyzerNodeEditor;
use crate::components::node_editor::optical_node_editor::OpticalNodeEditor;
use crate::components::scenery_editor::{NodeElement, NodeType};
use crate::{HTTP_API_CLIENT, OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_backend::{AnalyzerType, Fluence, Isometry, Properties, Proptype};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum NodeChange {
    Name(String),
    Lidt(Fluence),
    Alignment(Isometry),
    Inverted(bool),
    Property(String, Proptype),
    Isometry(Isometry),
    AnalyzerType(AnalyzerType),
}

#[component]
pub fn NodeConfigEditor(mut node_element_sig: Signal<Option<NodeElement>>) -> Element {
    let node_change = use_context_provider(|| Signal::new(None::<NodeChange>));
    let node_properties_sig = use_signal(Properties::default);
    let active_node_opt = node_element_sig();
    use_effect(move || {
        let node_change_opt = node_change.read().clone();
        if let (Some(node_changed), Some(active_node)) = (node_change_opt, active_node_opt.clone())
        {
            node_change_api_call_selection(
                node_changed,
                active_node,
                node_element_sig,
                node_properties_sig,
            );
        }
    });
    (*node_element_sig.read()).as_ref().map_or_else(
        || {
            rsx! {
                div { "No node selected" }
            }
        },
        |active_node| match active_node.node_type() {
            NodeType::Optical(_) => {
                rsx! {
                    OpticalNodeEditor { node_element_sig, node_properties_sig }
                }
            }
            NodeType::Analyzer(_) => {
                rsx! {
                    AnalyzerNodeEditor { node_element_sig }
                }
            }
        },
    )
}

fn node_change_api_call_selection(
    node_changed: NodeChange,
    mut active_node: NodeElement,
    mut node: Signal<Option<NodeElement>>,
    mut node_properties_sig: Signal<Properties>,
) {
    match node_changed {
        NodeChange::Name(name) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_name(&HTTP_API_CLIENT(), active_node.id(), name.clone()).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                } else {
                    active_node.set_name(name);
                    node.set(Some(active_node));
                }
            });
        }
        NodeChange::Lidt(lidt) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_lidt(&HTTP_API_CLIENT(), active_node.id(), lidt).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                }
            });
        }
        NodeChange::Alignment(iso) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_alignment(&HTTP_API_CLIENT(), active_node.id(), iso).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                }
            });
        }
        NodeChange::Property(key, prop) => {
            spawn(async move {
                if let Err(err_str) = api::update_node_property(
                    &HTTP_API_CLIENT(),
                    active_node.id(),
                    (key.clone(), prop.clone()),
                )
                .await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                } else {
                    node_properties_sig
                        .write()
                        .set(&key, prop)
                        .unwrap_or_else(|_| {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(&format!("Failed to set property: {key}"));
                        });
                }
            });
        }
        NodeChange::Isometry(iso) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_isometry(&HTTP_API_CLIENT(), active_node.id(), iso).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                }
            });
        }
        NodeChange::Inverted(inverted) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_inversion(&HTTP_API_CLIENT(), active_node.id(), inverted).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                } else {
                    active_node.set_inverted(inverted);
                    node.set(Some(active_node));
                }
            });
        }
        NodeChange::AnalyzerType(analyzer_type) => {
            spawn(async move {
                if let Err(err_str) = api::update_analyzer_config_ron(
                    &HTTP_API_CLIENT(),
                    active_node.id(),
                    analyzer_type,
                )
                .await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                }
            });
        }
    }
}
