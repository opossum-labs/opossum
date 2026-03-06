use crate::components::node_editor::analyzer_node_editor::AnalyzerNodeEditor;
use crate::components::node_editor::hooks::use_save_manager;
use crate::components::node_editor::inputs::input_components::FormContext;
use crate::components::node_editor::optical_node_editor::OpticalNodeEditor;
use crate::components::scenery_editor::{ActiveNode, GraphsWorkspaceAction, NodeType};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use futures_util::StreamExt;
use opossum_core::nodes::fluence_detector::Fluence;
use opossum_core::prelude::{AnalyzerType, Isometry, Proptype};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeChangeEvent {
    pub node_id: Uuid,
    pub action: NodeChangeAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeChangeAction {
    Name { name: String, graph_id: Uuid },
    Lidt(Fluence),
    Alignment(Isometry),
    Inverted { inverted: bool, graph_id: Uuid },
    Property(String, Proptype),
    Isometry(Option<Isometry>),
    AnalyzerType(AnalyzerType),
}

#[component]
pub fn NodeConfigEditor(
    active_node_opt: Memo<Option<ActiveNode>>,
    model_modified_handler: EventHandler<bool>,
) -> Element {
    let save_manager = use_save_manager();
    let flush_trigger = save_manager.flush_trigger;
    let dirty_count = save_manager.dirty_count;

    use_context_provider(|| FormContext {
        flush_trigger,
        dirty_count,
    });

    #[allow(clippy::redundant_closure)]
    let mut displayed_node = use_signal(|| active_node_opt());

    let memo_active_node_id = use_memo(move || {
        displayed_node().unwrap_or_else(|| ActiveNode {
            node_id: Uuid::nil(),
            graph_id: Uuid::nil(),
            node_type: NodeType::Optical("dummy".to_string()),
        })
    });

    use_effect(move || {
        if *dirty_count.read() == 0 {
            displayed_node.set(active_node_opt());
        }
    });

    // Standard Processing
    use_node_config_processor(model_modified_handler);
    let node_config_processor = use_coroutine_handle::<NodeChangeEvent>();
    let on_node_change = EventHandler::new(move |evt: NodeChangeEvent| {
        node_config_processor.send(evt);
    });

    match displayed_node().map(|n| n.node_type) {
        Some(NodeType::Optical(_)) => rsx! {
            OpticalNodeEditor { active_node: memo_active_node_id, on_change: on_node_change }
        },
        Some(NodeType::Analyzer(_)) => rsx! {
            AnalyzerNodeEditor { active_node: memo_active_node_id, on_change: on_node_change }
        },
        None => rsx! {
            div { "No node selected" }
        },
    }
}

fn use_node_config_processor(is_modified_handler: EventHandler<bool>) {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    use_coroutine(
        move |mut rx: UnboundedReceiver<NodeChangeEvent>| async move {
            while let Some(event) = rx.next().await {
                let uuid = event.node_id;

                let result: Result<(), String> = match event.action {
                    NodeChangeAction::Name { name, graph_id } => {
                        api::update_node_name(uuid, name.clone()).await.map(|_| {
                            workspace_processor.send(GraphsWorkspaceAction::SetNodeName {
                                name,
                                graph_id,
                                node_id: uuid,
                                needs_saving: true
                            });
                        })
                    }
                    NodeChangeAction::Lidt(lidt_new) => {
                        api::update_node_lidt(uuid, lidt_new).await.map(|_| ())
                    }
                    NodeChangeAction::Alignment(iso) => {
                        api::update_node_alignment(uuid, iso).await.map(|_| ())
                    }
                    NodeChangeAction::Property(key, prop) => {
                        api::update_node_property(uuid, (key.clone(), prop.clone()))
                            .await
                            .map(|_| ())
                    }
                    NodeChangeAction::Isometry(iso) => {
                        api::update_node_isometry(uuid, iso).await.map(|_| ())
                    }
                    NodeChangeAction::Inverted { inverted, graph_id } => {
                        match api::update_node_inversion(uuid, inverted).await {
                            Ok(connections) => {
                                workspace_processor.send(GraphsWorkspaceAction::UpdateEdges {
                                    connections,
                                    graph_id,
                                });
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
