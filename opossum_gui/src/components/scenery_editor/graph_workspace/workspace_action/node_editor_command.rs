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
    Refresh,
    AutoLayout,
    CenterGraph,
    ZoomToFit,
    JumpToMappedPort {
        mapped_node_id: Uuid,
        parent: (Uuid, String),
    },
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
    /// Turn a node with a volume into an amplifier (see [`crate::components::context_menu::cx_menu::CxtCommand::MakeAmplifier`]).
    MakeAmplifier {
        node_id: Uuid,
        graph_id: Uuid,
    },
    Undo,
    Redo,
}

/// Dispatches [`NodeEditorCommand::DeleteAll`]: clears the root scenery and reopens a fresh unsaved tab.
fn dispatch_delete_all(workspace_processor: Coroutine<GraphsWorkspaceAction>) {
    workspace_processor.send(GraphsWorkspaceAction::DeleteRootScenery);
    workspace_processor.send(GraphsWorkspaceAction::AddRootSceneryTab {
        name: "unsaved".to_string(),
    });
}

/// Dispatches [`NodeEditorCommand::AddNode`].
fn dispatch_add_node(
    workspace_processor: Coroutine<GraphsWorkspaceAction>,
    node_type: String,
    graph_id: Uuid,
) {
    workspace_processor.send(GraphsWorkspaceAction::AddOpticNode {
        node_type,
        graph_id,
    });
}

/// Dispatches [`NodeEditorCommand::AddNodeRef`].
fn dispatch_add_node_ref(
    workspace_processor: Coroutine<GraphsWorkspaceAction>,
    new_ref_node: NewRefNode,
    graph_id: Uuid,
) {
    workspace_processor.send(GraphsWorkspaceAction::AddOpticReference {
        new_ref_node,
        graph_id,
    });
}

/// Dispatches [`NodeEditorCommand::AddAnalyzer`].
fn dispatch_add_analyzer(
    workspace_processor: Coroutine<GraphsWorkspaceAction>,
    analyzer_type: AnalyzerType,
    graph_id: Uuid,
) {
    workspace_processor.send(GraphsWorkspaceAction::AddAnalyzer {
        analyzer_type,
        graph_id,
    });
}

/// Dispatches [`NodeEditorCommand::AutoLayout`]: re-runs the layout, then fits the camera to it as
/// part of the same undo step (`merge_into_previous_undo: true`) so one undo reverts both together.
fn dispatch_auto_layout(workspace_processor: Coroutine<GraphsWorkspaceAction>, graph_id: Uuid) {
    workspace_processor.send(GraphsWorkspaceAction::OptimizeLayout { graph_id });
    workspace_processor.send(GraphsWorkspaceAction::ZoomToFit {
        graph_id,
        save_changes: true,
        // Part of Auto Layout: fold this fit into the node re-positioning above so a
        // single undo reverts the whole auto-layout, camera included.
        merge_into_previous_undo: true,
    });
}

/// Dispatches [`NodeEditorCommand::CenterGraph`].
fn dispatch_center_graph(workspace_processor: Coroutine<GraphsWorkspaceAction>, graph_id: Uuid) {
    workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
        graph_id,
        save_changes: false,
        record_undo: true,
    });
}

/// Dispatches [`NodeEditorCommand::ZoomToFit`].
fn dispatch_zoom_to_fit(workspace_processor: Coroutine<GraphsWorkspaceAction>, graph_id: Uuid) {
    workspace_processor.send(GraphsWorkspaceAction::ZoomToFit {
        graph_id,
        save_changes: false,
        merge_into_previous_undo: false,
    });
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
            NodeEditorCommand::DeleteAll => dispatch_delete_all(workspace_processor),
            NodeEditorCommand::AddNode(node_type) => {
                dispatch_add_node(workspace_processor, node_type, active_tab());
            }
            NodeEditorCommand::AddNodeRef(new_ref_node) => {
                dispatch_add_node_ref(workspace_processor, new_ref_node, active_tab());
            }
            NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                dispatch_add_analyzer(workspace_processor, analyzer_type, active_tab());
            }
            NodeEditorCommand::AutoLayout => {
                dispatch_auto_layout(workspace_processor, active_tab());
            }
            NodeEditorCommand::CenterGraph => {
                dispatch_center_graph(workspace_processor, active_tab());
            }
            NodeEditorCommand::ZoomToFit => {
                dispatch_zoom_to_fit(workspace_processor, active_tab());
            }
            NodeEditorCommand::LoadFile(path) => {
                workspace_processor.send(GraphsWorkspaceAction::LoadFromFile(path));
            }
            NodeEditorCommand::SaveFile(path) => {
                workspace_processor.send(GraphsWorkspaceAction::SaveToFile(path));
            }
            NodeEditorCommand::Refresh => {
                workspace_processor.send(GraphsWorkspaceAction::Refresh);
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
            NodeEditorCommand::MakeAmplifier { node_id, graph_id } => {
                workspace_processor
                    .send(GraphsWorkspaceAction::MakeAmplifier { node_id, graph_id });
            }
            NodeEditorCommand::JumpToMappedPort {
                mapped_node_id,
                parent,
            } => {
                workspace_processor.send(GraphsWorkspaceAction::JumpToMappedPort {
                    mapped_node_id,
                    parent,
                });
            }
            NodeEditorCommand::Undo => {
                workspace_processor.send(GraphsWorkspaceAction::Undo);
            }
            NodeEditorCommand::Redo => {
                workspace_processor.send(GraphsWorkspaceAction::Redo);
            }
        }
        node_editor_command_handler.call(None);
    }
}
