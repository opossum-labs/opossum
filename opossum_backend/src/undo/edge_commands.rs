//! `apply` bodies for the connection-editing [`Command`] variants: [`Command::AddEdge`],
//! [`Command::RemoveEdge`], [`Command::UpdateEdgeDistance`].
use opossum_core::{meter, opm_document::OpmDocument, types::api_types::ConnectInfo};
use uuid::Uuid;

use super::Command;
use crate::error::BackEndErrorResponse;

/// Updates a connection's distance. `old`/`new` are full `ConnectInfo`s (identical except for
/// `distance`) so the GUI can be told the resulting edge directly, without a partial reconstruction.
#[derive(Clone)]
pub struct UpdateEdgeDistance {
    pub group_id: Uuid,
    pub old: ConnectInfo,
    pub new: ConnectInfo,
}

/// One connection inside a group, as needed by [`Command::AddEdge`]/[`Command::RemoveEdge`] to either
/// (re)create or tear it down.
#[derive(Clone)]
pub struct EdgeSnapshot {
    /// The group the connection lives in.
    pub group_id: Uuid,
    /// The connection itself (endpoints, ports, distance).
    pub connect_info: ConnectInfo,
}

/// Connects `cmd.connect_info` inside `cmd.group_id`, returning the [`Command::RemoveEdge`] that undoes it.
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group or the connection is invalid (unknown
/// uuid/port, or already connected).
pub(super) fn apply_add_edge(
    document: &mut OpmDocument,
    cmd: EdgeSnapshot,
) -> Result<Command, BackEndErrorResponse> {
    let EdgeSnapshot {
        group_id,
        connect_info,
    } = cmd;
    document.scenery_mut().with_group_node_mut(group_id, |g| {
        g.connect_nodes(
            connect_info.src_uuid(),
            connect_info.src_port(),
            connect_info.target_uuid(),
            connect_info.target_port(),
            meter!(connect_info.distance()),
        )
    })??;
    Ok(Command::RemoveEdge(EdgeSnapshot {
        group_id,
        connect_info,
    }))
}

/// Disconnects `cmd.connect_info` inside `cmd.group_id`, returning the [`Command::AddEdge`] that undoes it.
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group or the connection doesn't exist.
pub(super) fn apply_remove_edge(
    document: &mut OpmDocument,
    cmd: EdgeSnapshot,
) -> Result<Command, BackEndErrorResponse> {
    let EdgeSnapshot {
        group_id,
        connect_info,
    } = cmd;
    document.scenery_mut().with_group_node_mut(group_id, |g| {
        g.disconnect_nodes(connect_info.src_uuid(), connect_info.src_port())
    })??;
    Ok(Command::AddEdge(EdgeSnapshot {
        group_id,
        connect_info,
    }))
}

/// Updates a connection's distance to `new.distance()`, returning the [`Command::UpdateEdgeDistance`]
/// that undoes it (`old`/`new` swapped).
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group or the connection doesn't exist.
pub(super) fn apply_update_edge_distance(
    document: &mut OpmDocument,
    cmd: UpdateEdgeDistance,
) -> Result<Command, BackEndErrorResponse> {
    let UpdateEdgeDistance { group_id, old, new } = cmd;
    document.scenery_mut().with_group_node_mut(group_id, |g| {
        g.update_connection_distance(new.src_uuid(), new.src_port(), meter!(new.distance()))
    })??;
    Ok(Command::UpdateEdgeDistance(UpdateEdgeDistance {
        group_id,
        old: new,
        new: old,
    }))
}
