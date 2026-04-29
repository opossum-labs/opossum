use crate::components::node_editor::analyzer_node_editor::AnalyzerNodeEditor;
use crate::components::node_editor::hooks::use_save_manager;
use crate::components::node_editor::inputs::input_components::FormContext;
use crate::components::node_editor::optical_node_editor::OpticalNodeEditor;
use crate::components::scenery_editor::{GraphsWorkspaceAction, NodeType, SelectedNode};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use futures_util::StreamExt;
use opossum_core::prelude::{AnalyzerType, Aperture, ApertureShape, Isometry, Proptype};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeChangeEvent {
    pub node_id: Uuid,
    pub action: NodeChangeAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeChangeAction {
    Name(String),
    // Lidt(Fluence),
    Alignment(Isometry),
    Inverted { inverted: bool, graph_id: Uuid },
    Property(String, Proptype),
    Isometry(Option<Isometry>),
    AnalyzerType(AnalyzerType),
    Aperture(Aperture)
}

#[component]
pub fn NodeConfigEditor(
    selected_nodes_memo: Memo<Vec<SelectedNode>>,
    model_modified_handler: EventHandler<bool>,
    workspace_processor: Coroutine<GraphsWorkspaceAction>,
    active_graph_id: ReadSignal<Uuid>,
) -> Element {
    let save_manager = use_save_manager();
    let flush_trigger = save_manager.flush_trigger;
    let dirty_count = save_manager.dirty_count;

    use_context_provider(|| FormContext {
        flush_trigger,
        dirty_count,
    });

    #[allow(clippy::redundant_closure)]
    let mut displayed_nodes = use_signal(|| selected_nodes_memo());

    let memo_active_node_id = use_memo(move || {
        displayed_nodes()
            .first()
            .cloned()
            .unwrap_or_else(|| SelectedNode {
                node_id: Uuid::nil(),
                graph_id: Uuid::nil(),
                node_type: NodeType::Optical("dummy".to_string()),
            })
    });

    use_effect(move || {
        if *dirty_count.read() == 0 {
            displayed_nodes.set(selected_nodes_memo());
        }
    });

    // Standard Processing
    use_node_config_processor(model_modified_handler);
    let node_config_processor = use_coroutine_handle::<NodeChangeEvent>();
    let on_node_change = EventHandler::new(move |evt: NodeChangeEvent| {
        node_config_processor.send(evt);
    });

    if displayed_nodes.len() == 1 {
        match displayed_nodes().first().map(|n| n.node_type.clone()) {
            Some(NodeType::Optical(_)) => rsx! {
                OpticalNodeEditor {
                    active_node: memo_active_node_id,
                    on_change: on_node_change,
                }
            },
            Some(NodeType::Analyzer(_)) => rsx! {
                AnalyzerNodeEditor {
                    active_node: memo_active_node_id,
                    on_change: on_node_change,
                }
            },
            None => rsx! {
                div { "No node selected" }
            },
        }
    } else if displayed_nodes.len() == 0 {
        rsx! {
            div { "No node selected" }
        }
    } else {
        rsx! {
            div {
                "Multiple nodes selected"
                button {
                    class: "btn btn-success",
                    onclick: move |_| {
                        workspace_processor
                            .send(GraphsWorkspaceAction::ConvertToGroup {
                                nodes: selected_nodes_memo()
                                    .iter()
                                    .filter(|n| matches!(n.node_type, NodeType::Optical(_)))
                                    .map(|n| n.node_id)
                                    .collect::<Vec<Uuid>>(),
                                graph_id: *active_graph_id.read(),
                            });
                    },
                    "Convert nodes to group"
                }
            }
        }
    }
}

fn use_node_config_processor(is_modified_handler: EventHandler<bool>) {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    use_coroutine(
        move |mut rx: UnboundedReceiver<NodeChangeEvent>| async move {
            while let Some(event) = rx.next().await {
                let uuid = event.node_id;

                let result: Result<(), String> = match event.action {
                    NodeChangeAction::Name(name) => match api::get_node_references(uuid).await {
                        Ok(node_refs_grouped) => {
                            let ref_name = format!("ref ({name})");
                            for (group_id, ref_ids) in &node_refs_grouped {
                                for ref_id in ref_ids {
                                    let new_name = if uuid == *ref_id { &name } else { &ref_name };
                                    if let Err(e) =
                                        api::update_node_name(*ref_id, new_name).await.map(|()| {
                                            workspace_processor.send(
                                                GraphsWorkspaceAction::SetNodeName {
                                                    name: new_name.clone(),
                                                    graph_id: *group_id,
                                                    node_id: *ref_id,
                                                    needs_saving: true,
                                                },
                                            );
                                        })
                                    {
                                        OPOSSUM_UI_LOGS.write().add_log(&e);
                                    }
                                }
                            }
                            Ok(())
                        }
                        Err(e) => Err(e),
                    },
                    NodeChangeAction::Alignment(iso) => api::update_node_alignment(uuid, iso).await,
                    NodeChangeAction::Property(key, prop) => {
                        api::update_node_property(uuid, (key.clone(), prop.clone())).await
                    }
                    NodeChangeAction::Isometry(iso) => api::update_node_isometry(uuid, iso).await,
                    NodeChangeAction::Inverted { inverted, graph_id } => {
                        match api::update_node_inversion(uuid, inverted).await {
                            Ok(()) => {
                                // FIX THIS !!!!
                                // workspace_processor.send(GraphsWorkspaceAction::UpdateEdges {
                                //     connections,
                                //     graph_id,
                                // });
                                workspace_processor.send(GraphsWorkspaceAction::InvertNode {
                                    inverted,
                                    graph_id,
                                    node_id: uuid,
                                });
                                Ok(())
                            }
                            Err(e) => Err(e),
                        }
                    }
                    NodeChangeAction::AnalyzerType(analyzer_type) => {
                        api::update_analyzer_config_ron(uuid, analyzer_type)
                            .await
                            .map(|_| ())
                    }
                    NodeChangeAction::Aperture(aperture) => {
                        Ok(())
                        // todo!
                        // api::update_aperture_config_ron(uuid, aperture)
                        //     .await
                    },
                };

                match result {
                    Ok(()) => {
                        is_modified_handler.call(true);
                    }
                    Err(err_str) => {
                        OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    }
                }
            }
        },
    );
}
