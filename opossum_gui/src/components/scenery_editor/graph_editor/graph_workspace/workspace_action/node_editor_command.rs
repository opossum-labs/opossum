#![allow(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;
use opossum_core::{prelude::*, types::api_types::NewRefNode};
use std::path::PathBuf;
use uuid::Uuid;

use crate::components::scenery_editor::GraphsWorkspaceAction;
#[derive(Debug, Clone, PartialEq)]
pub enum NodeEditorCommand {
    DeleteAll,
    AddNode(String),
    AddNodeRef(NewRefNode),
    AddAnalyzer(AnalyzerType),
    LoadFile(PathBuf),
    SaveFile(PathBuf),
    AutoLayout,
    CenterGraph,
    ZoomToFit,
    ConvertToGroup {
        nodes: Vec<Uuid>,
        graph_id: Uuid,
    },
    MapNodePort {
        port_type: PortType,
        group_port_name: String,
        mapped_node_port_name: String,
        mapped_node_id: Uuid,
        group_id: Uuid,
    },
    RemovePortMap {
        group_id: Uuid,
        group_port_name: String,
        port_type: PortType,
    },
}

pub fn node_editor_command(
    node_editor_command_handler: EventHandler<Option<NodeEditorCommand>>,
    active_tab: ReadSignal<Uuid>,
    workspace_processor: Coroutine<GraphsWorkspaceAction>,
    command: ReadSignal<Option<NodeEditorCommand>>,
) {
    let cmd = command.read().clone();
    if let Some(command) = cmd {
        match command {
            NodeEditorCommand::DeleteAll => {
                workspace_processor.send(GraphsWorkspaceAction::DeleteRootScenery);
                workspace_processor.send(GraphsWorkspaceAction::AddRootSceneryTab {
                    name: "unsaved".to_string(),
                });
            }
            NodeEditorCommand::AddNode(node_type) => {
                workspace_processor.send(GraphsWorkspaceAction::AddOpticNode {
                    node_type,
                    graph_id: active_tab(),
                });
            }
            NodeEditorCommand::AddNodeRef(new_ref_node) => {
                workspace_processor.send(GraphsWorkspaceAction::AddOpticReference {
                    new_ref_node,
                    graph_id: active_tab(),
                });
            }
            NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                workspace_processor.send(GraphsWorkspaceAction::AddAnalyzer {
                    analyzer_type,
                    graph_id: active_tab(),
                });
            }
            NodeEditorCommand::AutoLayout => {
                workspace_processor.send(GraphsWorkspaceAction::OptimizeLayout {
                    graph_id: active_tab(),
                });
                workspace_processor.send(GraphsWorkspaceAction::ZoomToFit {
                    graph_id: active_tab(),
                    save_changes: true,
                });
            }
            NodeEditorCommand::CenterGraph => {
                workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
                    graph_id: active_tab(),
                    save_changes: true,
                });
            }
            NodeEditorCommand::ZoomToFit => {
                workspace_processor.send(GraphsWorkspaceAction::ZoomToFit {
                    graph_id: active_tab(),
                    save_changes: true,
                });
            }
            NodeEditorCommand::LoadFile(path) => {
                workspace_processor.send(GraphsWorkspaceAction::LoadFromFile(path));
            }
            NodeEditorCommand::SaveFile(path) => {
                workspace_processor.send(GraphsWorkspaceAction::SaveToFile(path));
            }
            NodeEditorCommand::ConvertToGroup { nodes, graph_id } => {
                workspace_processor.send(GraphsWorkspaceAction::ConvertToGroup { nodes, graph_id });
            }
            NodeEditorCommand::MapNodePort {
                port_type,
                group_port_name,
                mapped_node_port_name,
                mapped_node_id,
                group_id,
            } => {
                workspace_processor.send(GraphsWorkspaceAction::MapNodePort {
                    port_type,
                    group_port_name,
                    mapped_node_port_name,
                    mapped_node_id,
                    group_id,
                });
            }
            NodeEditorCommand::RemovePortMap {
                group_id,
                group_port_name,
                port_type,
            } => {
                workspace_processor.send(GraphsWorkspaceAction::RemovePortMap {
                    group_id,
                    group_port_name,
                    port_type,
                });
            }
        }
        node_editor_command_handler.call(None);
    }
}
