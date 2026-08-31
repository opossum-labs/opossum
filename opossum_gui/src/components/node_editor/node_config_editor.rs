use crate::components::node_editor::analyzer_node_editor::AnalyzerNodeEditor;
use crate::components::node_editor::hooks::use_save_manager;
use crate::components::node_editor::inputs::input_components::FormContext;
use crate::components::node_editor::optical_node_editor::OpticalNodeEditor;
use crate::components::scenery_editor::{GraphsWorkspaceAction, NodeType, SelectedNode};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use futures_util::StreamExt;
use opossum_core::core_optics::PortType;
use opossum_core::prelude::{AnalyzerType, Isometry, Proptype};
use opossum_core::types::api_types::UpdatePortRequest;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeChangeEvent {
    pub node_id: Uuid,
    pub action: NodeChangeAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeChangeAction {
    Name(String),
    Alignment(Isometry),
    Inverted {
        inverted: bool,
        graph_id: Uuid,
    },
    Property(String, Proptype),
    Isometry(Option<Isometry>),
    AnalyzerType(AnalyzerType),
    /// The pump scenarios an analyzer is run in, in the given order - one report per entry, and a
    /// single passive run when empty.
    ///
    /// Separate from [`NodeChangeAction::AnalyzerType`] because the selection sits next to the
    /// analyzer's config rather than inside it: it names operating points of the document, which is
    /// why it has an endpoint (and an undo command) of its own on the backend.
    AnalyzerPumpScenarios(Vec<Uuid>),
    PortConfig {
        port_name: String,
        port_type: PortType,
        request: UpdatePortRequest,
    },
}

#[component]
pub fn NodeConfigEditor(
    selected_nodes_memo: Memo<Vec<SelectedNode>>,
    model_modified_handler: EventHandler<bool>,
    workspace_processor: Coroutine<GraphsWorkspaceAction>,
    active_graph_id: ReadSignal<Uuid>,
) -> Element {
    info!("🔄 Render: NodeConfigEditor");
    let save_manager = use_save_manager();
    let flush_trigger = save_manager.flush_trigger;
    let dirty_count = save_manager.dirty_count;

    use_context_provider(|| FormContext {
        flush_trigger,
        dirty_count,
    });

    // Stores the last confirmed selection while the form is clean
    let mut last_clean_selection = use_signal(&*selected_nodes_memo);

    // Synchronously derive displayed nodes during render to prevent double-render cycles
    let displayed_nodes = use_memo(move || {
        let is_dirty = *dirty_count.read() > 0;
        if is_dirty {
            // Keep existing selection locked while editing
            last_clean_selection.read().clone()
        } else {
            // Always reflect the latest selection immediately
            selected_nodes_memo()
        }
    });

    use_effect(move || {
        let current_selection = selected_nodes_memo();
        if *dirty_count.read() == 0
            && last_clean_selection.peek().as_slice() != current_selection.as_slice()
        {
            last_clean_selection.set(current_selection);
        }
    });

    // --- NEW: Granular memos to prevent unnecessary re-renders in child components ---

    // Extract only the node_id. Child will only re-render if the UUID actually changes.
    let memo_node_id = use_memo(move || {
        displayed_nodes()
            .first()
            .map_or_else(Uuid::nil, |n| n.node_id)
    });

    // Extract only the graph_id.
    let memo_graph_id = use_memo(move || {
        displayed_nodes()
            .first()
            .map_or_else(Uuid::nil, |n| n.graph_id)
    });

    // Extract the node_type.
    let memo_node_type = use_memo(move || displayed_nodes().first().map(|n| n.node_type.clone()));

    // --- TRANSITION ONLY: Kept for AnalyzerNodeEditor until we refactor it ---
    let legacy_memo_active_node = use_memo(move || {
        displayed_nodes()
            .first()
            .cloned()
            .unwrap_or_else(|| SelectedNode {
                node_id: Uuid::nil(),
                graph_id: Uuid::nil(),
                node_type: NodeType::Optical("dummy".to_string()),
            })
    });

    // Standard Processing
    use_node_config_processor(model_modified_handler);
    let node_config_processor = use_coroutine_handle::<NodeChangeEvent>();
    let on_change = use_callback(move |evt: NodeChangeEvent| {
        node_config_processor.send(evt);
    });

    if displayed_nodes.len() == 1 {
        match memo_node_type() {
            Some(NodeType::Optical(_)) => rsx! {
                OpticalNodeEditor {
                    // Pass the granular memos directly
                    node_id: memo_node_id,
                    graph_id: memo_graph_id,
                    on_change,
                }
            },
            Some(NodeType::Analyzer(_)) => rsx! {
                AnalyzerNodeEditor {
                    // We still use the legacy memo here until we refactor AnalyzerNodeEditor
                    active_node: legacy_memo_active_node,
                    on_change,
                }
            },
            None => rsx! {
                div { class: "noselect", "No node selected" }
            },
        }
    } else if displayed_nodes.len() == 0 {
        rsx! {
            div { class: "noselect", "No node selected" }
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
                // The amplifier overview lists nodes by name, so a rename has to reach it. Every
                // other way that list can change already bumps the counter in the workspace
                // processor; taking the flag here keeps this out of the generic property path.
                let is_rename = matches!(event.action, NodeChangeAction::Name(_));

                let result: Result<(), String> = match event.action {
                    NodeChangeAction::Name(name) => match api::get_node_references(uuid).await {
                        Ok(node_refs_grouped) => {
                            // Send a single rename; the backend propagates it to reference nodes as one
                            // undo step (see `patch_node`). We only fan out the *local* canvas display
                            // for the node and its references here - no extra backend PATCHes.
                            match api::update_node_name(uuid, &name).await {
                                Ok(()) => {
                                    let ref_name = format!("ref ({name})");
                                    for (group_id, ref_ids) in &node_refs_grouped {
                                        for ref_id in ref_ids {
                                            let new_name = if uuid == *ref_id {
                                                name.clone()
                                            } else {
                                                ref_name.clone()
                                            };
                                            workspace_processor.send(
                                                GraphsWorkspaceAction::SetNodeName {
                                                    name: new_name,
                                                    graph_id: *group_id,
                                                    node_id: *ref_id,
                                                    needs_saving: true,
                                                },
                                            );
                                        }
                                    }
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
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
                        api::update_analyzer_config_ron(uuid, analyzer_type).await
                    }
                    NodeChangeAction::AnalyzerPumpScenarios(scenarios) => {
                        api::put_analyzer_pump_scenarios(uuid, scenarios).await
                    }
                    NodeChangeAction::PortConfig {
                        port_name,
                        port_type,
                        request,
                    } => api::patch_node_port_config(uuid, port_name, port_type, request).await,
                };

                match result {
                    Ok(()) => {
                        is_modified_handler.call(true);
                        // Keep the properties panel's own fetched data (node_info_sig etc., not
                        // mirrored into GraphStore) in sync with what was just saved - without this,
                        // it only reflects the backend's truth after an undo/redo-triggered refetch,
                        // never after a normal direct edit.
                        *crate::NODE_DETAILS_REFRESH.write() += 1;
                        if is_rename {
                            *crate::AMP_LIST_REFRESH.write() += 1;
                        }
                        // The edit pushed an undo entry on the backend; reflect that in the Edit menu.
                        *crate::UNDO_REDO_STATUS.write() = (true, false);
                    }
                    Err(err_str) => {
                        OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    }
                }
            }
        },
    );
}
