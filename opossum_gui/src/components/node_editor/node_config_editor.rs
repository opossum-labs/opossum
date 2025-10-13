#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::analyzer_node_editor::AnalyzerNodeEditor;
use crate::components::node_editor::optical_node_editor::OpticalNodeEditor;
use crate::components::scenery_editor::{GraphStore, GraphStoreAction, NodeType};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use futures_util::StreamExt;
use opossum_backend::{AnalyzerType, Fluence, Isometry, Properties, Proptype};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum NodeChangeAction {
    Name(String),
    Lidt(Fluence),
    Alignment(Isometry),
    Inverted(bool),
    Property(String, Proptype),
    Isometry(Option<Isometry>),
    AnalyzerType(AnalyzerType),
}

#[component]
pub fn NodeConfigEditor(
    active_node_opt: Memo<Option<(NodeType, Uuid)>>,
    is_modified: Signal<bool>,
) -> Element {
    let node_properties_sig = use_signal(Properties::default);

    use_context_provider(|| node_properties_sig);
    use_node_config_processor(node_properties_sig, is_modified);

    (active_node_opt()).map_or_else(
        || {
            rsx! {
                div { "No node selected" }
            }
        },
        |(active_node_type, node_id)| match active_node_type {
            NodeType::Optical(_) => {
                rsx! {
                    OpticalNodeEditor { node_id, node_properties_sig }
                }
            }
            NodeType::Analyzer(_) => {
                rsx! {
                    AnalyzerNodeEditor { node_id }
                }
            }
        },
    )
}
#[allow(clippy::too_many_lines)]
fn use_node_config_processor(
    mut node_properties_sig: Signal<Properties>,
    mut is_modified: Signal<bool>,
) {
    let graph_processor = use_coroutine_handle::<GraphStoreAction>();
    let mut graph_store = use_context::<Signal<GraphStore>>();
    use_coroutine(move |mut rx: UnboundedReceiver<NodeChangeAction>| {
        async move {
            // This loop runs forever in the background, waiting for actions.
            while let Some(action) = rx.next().await {
                if let Some(active_node_id) = graph_store.read().active_node() {
                    match action {
                        NodeChangeAction::Name(name) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_node_name(active_node_id, name.clone()).await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                } else {
                                    is_modified.set(true);
                                    graph_store.write().set_name_of_node(active_node_id, name);
                                }
                            });
                        }
                        NodeChangeAction::Lidt(lidt) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_node_lidt(active_node_id, lidt).await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                } else {
                                    is_modified.set(true);
                                }
                            });
                        }
                        NodeChangeAction::Alignment(iso) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_node_alignment(active_node_id, iso).await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                } else {
                                    is_modified.set(true);
                                }
                            });
                        }
                        NodeChangeAction::Property(key, prop) => {
                            spawn(async move {
                                if let Err(err_str) = api::update_node_property(
                                    active_node_id,
                                    (key.clone(), prop.clone()),
                                )
                                .await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                } else {
                                    //needed for grating alignment menu
                                    node_properties_sig.write().set(&key, prop).unwrap_or_else(
                                        |_| {
                                            OPOSSUM_UI_LOGS
                                                .write()
                                                .add_log(&format!("Failed to set property: {key}"));
                                        },
                                    );
                                    is_modified.set(true);
                                }
                            });
                        }
                        NodeChangeAction::Isometry(iso) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_node_isometry(active_node_id, iso).await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                } else {
                                    is_modified.set(true);
                                }
                            });
                        }
                        NodeChangeAction::Inverted(inverted) => {
                            spawn(async move {
                                match api::update_node_inversion(active_node_id, inverted).await {
                                    Ok(connections) => {
                                        graph_processor
                                            .send(GraphStoreAction::UpdateEdges(connections));
                                        graph_store
                                            .write()
                                            .set_node_inverted(active_node_id, inverted);
                                        is_modified.set(true);
                                    }
                                    Err(err_str) => {
                                        OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                    }
                                }
                            });
                        }
                        NodeChangeAction::AnalyzerType(analyzer_type) => {
                            spawn(async move {
                                if let Err(err_str) =
                                    api::update_analyzer_config_ron(active_node_id, analyzer_type)
                                        .await
                                {
                                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                                } else {
                                    is_modified.set(true);
                                }
                            });
                        }
                    }
                }
            }
        }
    });
}
