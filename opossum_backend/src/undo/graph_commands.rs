//! Execution, inversion, and description logic for graph mutations represented by [`GraphDelta`].

use opossum_core::{
    core_optics::OpticRef,
    nodes::{
        GraphDelta,
        node_group::optic_graph::delta::{GraphDeletionDelta, RemovedPortMapping},
    },
    opm_document::OpmDocument,
    prelude::PortType,
    types::api_types::{ConnectInfo, DocumentChange, JumpTarget, NodeInfo},
};
use uuid::Uuid;

use super::Command;
use crate::{error::BackEndErrorResponse, helper_functions::parent_group_id_or_self};

/// Helper function to convert an [`OpticRef`] into the [`NodeInfo`] expected by GUI updates.
fn node_info(node: &OpticRef) -> NodeInfo {
    NodeInfo::from_analyzable(&**node, None)
}

/// Applies an undo step for a [`GraphDelta`] mutation.
///
/// Dispatches directly to [`NodeGroup::apply_undo`] in `opossum_core`, which reverses
/// the topological change. Returns the corresponding [`Command::RedoGraph`] containing
/// the delta to be pushed onto the redo stack.
///
/// # Errors
/// Returns [`BackEndErrorResponse`] if restoring any node, edge, or port mapping fails.
pub(super) fn apply_undo_graph(
    document: &mut OpmDocument,
    delta: GraphDelta,
) -> Result<Command, BackEndErrorResponse> {
    document
        .scenery_mut()
        .apply_undo(&delta)
        .map_err(|e| BackEndErrorResponse::new(500, "Undo Error", &e.to_string()))?;

    Ok(Command::RedoGraph(Box::new(delta)))
}

/// Re-executes a forward graph mutation during redo.
///
/// Repeats the forward action described by `delta`, returning a fresh [`Command::UndoGraph`]
/// with the newly generated delta for subsequent undo cycles.
///
/// # Errors
/// Returns [`BackEndErrorResponse`] if applying the forward mutation fails.
pub(super) fn apply_redo_graph(
    document: &mut OpmDocument,
    delta: GraphDelta,
) -> Result<Command, BackEndErrorResponse> {
    let fresh_delta = match delta {
        GraphDelta::NodeAdded { group_id, node, .. } => {
            let (_, d) = document
                .scenery_mut()
                .with_group_node_mut(group_id, |g| g.add_node_ref_with_delta(node))??;
            d
        }
        GraphDelta::NodeDeleted(deletion_delta) => document
            .scenery_mut()
            .delete_node(deletion_delta.target_node_id)?,
        GraphDelta::NodesConnected {
            group_id,
            connection,
            ..
        } => document.scenery_mut().with_group_node_mut(group_id, |g| {
            g.connect_nodes(
                connection.src_id,
                &connection.src_port,
                connection.target_id,
                &connection.target_port,
                connection.distance,
            )
        })??,
        GraphDelta::NodesDisconnected {
            group_id,
            connection,
        } => document.scenery_mut().with_group_node_mut(group_id, |g| {
            g.disconnect_nodes(connection.src_id, &connection.src_port)
        })??,
        GraphDelta::ConnectionDistanceChanged {
            group_id,
            src_id,
            src_port,
            new_distance,
            ..
        } => document.scenery_mut().with_group_node_mut(group_id, |g| {
            g.update_connection_distance(src_id, &src_port, new_distance)
        })??,
        GraphDelta::PortMapped(mapping) => {
            document
                .scenery_mut()
                .with_group_node_mut(mapping.group_id, |g| match mapping.port_type {
                    PortType::Input => g.map_input_port(
                        mapping.internal_node_id,
                        &mapping.internal_port_name,
                        &mapping.external_name,
                    ),
                    PortType::Output => g.map_output_port(
                        mapping.internal_node_id,
                        &mapping.internal_port_name,
                        &mapping.external_name,
                    ),
                })??
        }
        GraphDelta::PortUnmapped(mapping) => document
            .scenery_mut()
            .with_group_node_mut(mapping.group_id, |g| {
                g.remove_mapped_port(&mapping.external_name, mapping.port_type)
            })??,
        GraphDelta::Composite(sub_deltas) => {
            let mut fresh_sub_deltas = Vec::with_capacity(sub_deltas.len());
            for sub in sub_deltas {
                let sub_cmd = apply_redo_graph(document, sub)?;
                if let Command::UndoGraph(boxed_fresh) = sub_cmd {
                    fresh_sub_deltas.push(*boxed_fresh);
                }
            }
            GraphDelta::Composite(fresh_sub_deltas)
        }
    };

    Ok(Command::UndoGraph(Box::new(fresh_delta)))
}

/// Describes the effect of a [`GraphDelta`] on the canvas as [`DocumentChange`] events.
pub(super) fn describe_graph_delta(
    delta: &GraphDelta,
    is_undo: bool,
    document: &OpmDocument,
) -> Vec<DocumentChange> {
    match delta {
        GraphDelta::NodeAdded {
            group_id,
            node_id,
            node,
        } => {
            if is_undo {
                vec![DocumentChange::NodeRemoved {
                    graph_id: *group_id,
                    uuid: *node_id,
                }]
            } else {
                vec![DocumentChange::NodeAdded {
                    graph_id: *group_id,
                    node: Box::new(node_info(node)),
                }]
            }
        }
        GraphDelta::NodeDeleted(deletion_delta) => {
            if is_undo {
                describe_restored_deletion(deletion_delta)
            } else {
                describe_reapplied_deletion(deletion_delta)
            }
        }
        GraphDelta::NodesConnected {
            group_id,
            connection,
            displaced_port_mappings,
        } => {
            let connect_info = ConnectInfo::from_connection_info(connection, false);
            let mut changes = if is_undo {
                vec![DocumentChange::EdgeRemoved {
                    graph_id: *group_id,
                    connect_info,
                }]
            } else {
                vec![DocumentChange::EdgeAdded {
                    graph_id: *group_id,
                    connect_info,
                }]
            };

            for mapping in displaced_port_mappings {
                changes.push(DocumentChange::GraphNeedsRefresh {
                    graph_id: mapping.group_id,
                });
            }
            changes
        }
        GraphDelta::NodesDisconnected {
            group_id,
            connection,
        } => {
            let connect_info = ConnectInfo::from_connection_info(connection, false);
            if is_undo {
                vec![DocumentChange::EdgeAdded {
                    graph_id: *group_id,
                    connect_info,
                }]
            } else {
                vec![DocumentChange::EdgeRemoved {
                    graph_id: *group_id,
                    connect_info,
                }]
            }
        }
        GraphDelta::ConnectionDistanceChanged { group_id, .. } => {
            vec![DocumentChange::GraphNeedsRefresh {
                graph_id: *group_id,
            }]
        }
        GraphDelta::PortMapped(mapping) | GraphDelta::PortUnmapped(mapping) => {
            let parent_id = parent_group_id_or_self(document.scenery(), mapping.group_id)
                .unwrap_or(mapping.group_id);
            vec![
                DocumentChange::GraphNeedsRefresh {
                    graph_id: mapping.group_id,
                },
                DocumentChange::GraphNeedsRefresh {
                    graph_id: parent_id,
                },
            ]
        }
        GraphDelta::Composite(deltas) => {
            let mut changes = Vec::new();
            if is_undo {
                for sub in deltas.iter().rev() {
                    changes.extend(describe_graph_delta(sub, is_undo, document));
                }
            } else {
                for sub in deltas {
                    changes.extend(describe_graph_delta(sub, is_undo, document));
                }
            }
            changes
        }
    }
}

fn describe_restored_deletion(deletion_delta: &GraphDeletionDelta) -> Vec<DocumentChange> {
    let mut changes = Vec::new();

    for record in &deletion_delta.deleted_nodes {
        changes.push(DocumentChange::NodeAdded {
            graph_id: record.parent_group_id,
            node: Box::new(node_info(&record.node)),
        });
    }

    for conn_rec in &deletion_delta.removed_connections {
        let is_ref = deletion_delta.deleted_nodes.iter().any(|r| {
            r.node.uuid() == conn_rec.connection.target_id
                && r.node.node_attr().properties().get("reference id").is_ok()
        });
        let connect_info = ConnectInfo::from_connection_info(&conn_rec.connection, is_ref);
        changes.push(DocumentChange::EdgeAdded {
            graph_id: conn_rec.parent_group_id,
            connect_info,
        });
    }

    for mapping in &deletion_delta.removed_port_mappings {
        changes.push(DocumentChange::GraphNeedsRefresh {
            graph_id: mapping.group_id,
        });
    }

    changes
}

fn describe_reapplied_deletion(deletion_delta: &GraphDeletionDelta) -> Vec<DocumentChange> {
    let mut changes = Vec::new();

    for record in &deletion_delta.deleted_nodes {
        changes.push(DocumentChange::NodeRemoved {
            graph_id: record.parent_group_id,
            uuid: record.node.uuid(),
        });
    }

    for conn_rec in &deletion_delta.removed_connections {
        let is_ref = deletion_delta.deleted_nodes.iter().any(|r| {
            r.node.uuid() == conn_rec.connection.target_id
                && r.node.node_attr().properties().get("reference id").is_ok()
        });
        let connect_info = ConnectInfo::from_connection_info(&conn_rec.connection, is_ref);
        changes.push(DocumentChange::EdgeRemoved {
            graph_id: conn_rec.parent_group_id,
            connect_info,
        });
    }

    for mapping in &deletion_delta.removed_port_mappings {
        changes.push(DocumentChange::GraphNeedsRefresh {
            graph_id: mapping.group_id,
        });
    }

    changes
}

/// Determines the focus target tab/node on the canvas after undoing or redoing a [`GraphDelta`].
#[must_use]
pub(super) fn jump_target_for_graph_delta(
    delta: &GraphDelta,
    is_undo: bool,
    root_id: Uuid,
) -> Option<JumpTarget> {
    match delta {
        GraphDelta::NodeAdded {
            group_id, node_id, ..
        } => {
            if is_undo {
                Some(JumpTarget::new_from_graph_id(*group_id))
            } else {
                Some(JumpTarget::new_from_graph_and_node_id(*group_id, *node_id))
            }
        }
        GraphDelta::NodeDeleted(deletion_delta) => {
            let target_id = deletion_delta.target_node_id;
            let parent_group_id = deletion_delta
                .deleted_nodes
                .iter()
                .find(|record| record.node.uuid() == target_id)
                .map_or(root_id, |record| record.parent_group_id);

            if is_undo {
                Some(JumpTarget::new_from_graph_and_node_id(
                    parent_group_id,
                    target_id,
                ))
            } else {
                Some(JumpTarget::new_from_graph_id(parent_group_id))
            }
        }
        GraphDelta::NodesConnected { group_id, .. }
        | GraphDelta::NodesDisconnected { group_id, .. }
        | GraphDelta::ConnectionDistanceChanged { group_id, .. }
        | GraphDelta::PortMapped(RemovedPortMapping { group_id, .. })
        | GraphDelta::PortUnmapped(RemovedPortMapping { group_id, .. }) => {
            Some(JumpTarget::new_from_graph_id(*group_id))
        }
        GraphDelta::Composite(deltas) => {
            // Focus on the innermost port mapping (the cascade origin) if present
            deltas
                .iter()
                .rev()
                .find_map(|d| match d {
                    GraphDelta::PortMapped(m) | GraphDelta::PortUnmapped(m) => {
                        Some(JumpTarget::new_from_graph_id(m.group_id))
                    }
                    _ => None,
                })
                .or_else(|| {
                    deltas
                        .first()
                        .and_then(|d| jump_target_for_graph_delta(d, is_undo, root_id))
                })
        }
    }
}
