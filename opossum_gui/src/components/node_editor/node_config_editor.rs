#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::analyzer_node_editor::AnalyzerNodeEditor;
use crate::components::node_editor::optical_node_editor::OpticalNodeEditor;
use crate::components::scenery_editor::{NodeEditorCommand, NodeElement, NodeType};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use futures_util::StreamExt;
use opossum_backend::{AnalyzerType, Fluence, Isometry, Properties, Proptype};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum NodeChangeAction {
    Name(String),
    Lidt(Fluence),
    Alignment(Isometry),
    Inverted(bool),
    Property(String, Proptype),
    Isometry(Isometry),
    AnalyzerType(AnalyzerType),
}

#[component]
pub fn NodeConfigEditor(
    mut node_element_sig: Signal<Option<NodeElement>>,
    node_editor_command: Signal<Option<NodeEditorCommand>>,
) -> Element {
    let node_change = use_context_provider(|| Signal::new(None::<NodeChangeAction>));
    let node_properties_sig = use_signal(Properties::default);
    let node_config_processor = use_node_config_processor(node_element_sig, node_properties_sig);
    use_effect(move || {
        if let Some(node_change_action) = node_change() {
            node_config_processor.send(node_change_action);
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

fn use_node_config_processor(
    mut node_selected: Signal<Option<NodeElement>>,
    mut node_properties_sig: Signal<Properties>,
) -> Coroutine<NodeChangeAction> {
    use_coroutine(move |mut rx: UnboundedReceiver<NodeChangeAction>| {
        async move {
            // This loop runs forever in the background, waiting for actions.
            while let Some(action) = rx.next().await {
                if let Some(active_node) = node_selected() {
                    match action {
                        NodeChangeAction::Name(name) => {
                            spawn(async move {
                                let mut active_node = active_node.clone();
                                if let Err(err_str) =
                                    api::update_node_name(active_node.id(), name.clone()).await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                } else {
                                    active_node.set_name(name);
                                    node_selected.set(Some(active_node));
                                }
                            });
                        }
                        NodeChangeAction::Lidt(lidt) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_node_lidt(active_node.id(), lidt).await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                }
                            });
                        }
                        NodeChangeAction::Alignment(iso) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_node_alignment(active_node.id(), iso).await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                }
                            });
                        }
                        NodeChangeAction::Property(key, prop) => {
                            spawn(async move {
                                if let Err(err_str) = api::update_node_property(
                                    active_node.id(),
                                    (key.clone(), prop.clone()),
                                )
                                .await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                } else {
                                    node_properties_sig.write().set(&key, prop).unwrap_or_else(
                                        |_| {
                                            OPOSSUM_UI_LOGS
                                                .write()
                                                .add_log(&format!("Failed to set property: {key}"));
                                        },
                                    );
                                }
                            });
                        }
                        NodeChangeAction::Isometry(iso) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_node_isometry(active_node.id(), iso).await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                }
                            });
                        }
                        NodeChangeAction::Inverted(inverted) => todo!(),
                        // {
                        //     spawn(async move {
                        //         match api::update_node_inversion(active_node.id(), inverted).await {
                        //             Ok(connections) => {
                        //                 node_editor_command.set(Some(NodeEditorCommand::UpdateEdges(connections)));
                        //                 active_node.set_inverted(inverted);
                        //                 node.set(Some(active_node));
                        //             }
                        //             Err(err_str) => {
                        //                 OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        //             }
                        //         }
                        //     });
                        // }
                        NodeChangeAction::AnalyzerType(analyzer_type) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_analyzer_config_ron(active_node.id(), analyzer_type)
                                        .await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                }
                            });
                        }
                    }
                }
            }
        }
    })
}
