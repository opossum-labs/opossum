//! `apply`/`describe` bodies for the group-structure [`Command`] variants: [`Command::MoveNodes`],
//! [`Command::InsertGroup`], [`Command::ExtractGroup`].
use std::collections::HashSet;

use opossum_core::{
    core_optics::OpticRef,
    error::OpossumError,
    nodes::NodeGroup,
    opm_document::OpmDocument,
    types::api_types::{ConnectInfo, DocumentChange, MoveNodesRequest},
    utils::LockExt,
};
use uuid::Uuid;

use super::Command;
use crate::{error::BackEndErrorResponse, helper_functions::connect_from_info};

/// Removes `member_ids` from `parent_group_id`'s flat graph and inserts the previously captured `group`
/// node in their place, reconnecting `external_connections` (in terms of the *group's own*
/// uuid/exposed-port-names) to its exposed ports. The group's internal members/connections are
/// untouched - they live inside `group`'s own nested graph the whole time, whether or not `group` is
/// currently attached to the document. `restore_connections` is carried through unchanged for the
/// [`ExtractGroup`] this produces - see its docs for why it needs a different, member-uuid-based
/// representation of the same connections.
#[derive(Clone)]
pub struct InsertGroup {
    pub parent_group_id: Uuid,
    pub group: OpticRef,
    pub member_ids: Vec<Uuid>,
    pub external_connections: Vec<ConnectInfo>,
    pub restore_connections: Vec<ConnectInfo>,
}

/// The inverse of [`InsertGroup`]: removes `group` from `parent_group_id` and re-inserts its members as
/// flat nodes in its place. Reconnects `restore_connections` - every connection that touched a member
/// *before* grouping (both formerly-internal and formerly-boundary), expressed in terms of the original
/// member uuids/ports - rather than `external_connections`, which references the group's own uuid and
/// can't be used once the group node has just been deleted.
#[derive(Clone)]
pub struct ExtractGroup {
    pub parent_group_id: Uuid,
    pub group: OpticRef,
    pub member_ids: Vec<Uuid>,
    pub external_connections: Vec<ConnectInfo>,
    pub restore_connections: Vec<ConnectInfo>,
}

/// Moves `request.nodes_to_move` (and the connections purely between them) from
/// `request.source_group_id` to `request.target_group_id`, returning the swapped [`Command::MoveNodes`]
/// that undoes it.
///
/// # Errors
///
/// Returns an error if either group id doesn't resolve, or a moved node's uuid can't be found.
pub(super) fn apply_move_nodes(
    document: &mut OpmDocument,
    request: MoveNodesRequest,
) -> Result<Command, BackEndErrorResponse> {
    let MoveNodesRequest {
        source_group_id: from_group_id,
        target_group_id: to_group_id,
        nodes_to_move: node_ids,
    } = request;

    let node_set: HashSet<Uuid> = node_ids.iter().copied().collect();
    let connections = document
        .scenery()
        .with_group_node(from_group_id, NodeGroup::connections)?;
    let inside = connections
        .iter()
        .filter(|c| node_set.contains(&c.src_id) && node_set.contains(&c.target_id))
        .map(|c| {
            let is_reference = document
                .scenery()
                .with_node_attr(c.target_id, |attr| {
                    attr.properties().get("reference id").is_ok()
                })
                .unwrap_or(false);
            ConnectInfo::from_connection_info(c, is_reference)
        })
        .collect::<Vec<_>>();

    let node_refs: Vec<OpticRef> = node_ids
        .iter()
        .filter_map(|id| document.scenery().node_recursive(*id).ok().map(|(r, _)| r))
        .collect();

    for id in &node_ids {
        document.scenery_mut().delete_node(*id)?;
    }
    for node_ref in &node_refs {
        document
            .scenery_mut()
            .with_group_node_mut(to_group_id, |g| g.add_node_ref(node_ref.clone()))??;
    }
    for conn in &inside {
        document
            .scenery_mut()
            .with_group_node_mut(to_group_id, |g| connect_from_info(g, conn))??;
    }

    Ok(Command::MoveNodes(MoveNodesRequest {
        source_group_id: to_group_id,
        target_group_id: from_group_id,
        nodes_to_move: node_ids,
    }))
}

/// Describes the effect of a [`Command::MoveNodes`] in the GUI-facing [`DocumentChange`] shape.
pub(super) fn describe_move_nodes(request: &MoveNodesRequest) -> Vec<DocumentChange> {
    vec![
        DocumentChange::GraphNeedsRefresh {
            graph_id: request.source_group_id,
        },
        DocumentChange::GraphNeedsRefresh {
            graph_id: request.target_group_id,
        },
    ]
}

/// Removes `member_ids` from `parent_group_id`'s flat graph and inserts the previously captured `group`
/// node in their place, reconnecting `external_connections` to its exposed ports. Returns the
/// [`Command::ExtractGroup`] that undoes it.
///
/// # Errors
///
/// Returns an error if `parent_group_id` doesn't resolve to a group or a member id can't be deleted.
pub(super) fn apply_insert_group(
    document: &mut OpmDocument,
    cmd: InsertGroup,
) -> Result<Command, BackEndErrorResponse> {
    let InsertGroup {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
    } = cmd;
    for member_id in &member_ids {
        document.scenery_mut().delete_node(*member_id)?;
    }
    document
        .scenery_mut()
        .with_group_node_mut(parent_group_id, |g| g.add_node_ref(group.clone()))??;
    for conn in &external_connections {
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| connect_from_info(g, conn))??;
    }
    Ok(Command::ExtractGroup(ExtractGroup {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
    }))
}

/// The inverse of [`apply_insert_group`]: removes `group` from `parent_group_id` and re-inserts its
/// members as flat nodes in its place, reconnecting `restore_connections`. Returns the
/// [`Command::InsertGroup`] that undoes it.
///
/// # Errors
///
/// Returns an error if `group`'s uuid can't be resolved, it isn't a `NodeGroup`, or a member id isn't
/// found inside it.
pub(super) fn apply_extract_group(
    document: &mut OpmDocument,
    cmd: ExtractGroup,
) -> Result<Command, BackEndErrorResponse> {
    let ExtractGroup {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
    } = cmd;
    let group_id = group.uuid()?;
    document.scenery_mut().delete_node(group_id)?;
    for member_id in &member_ids {
        let member_ref = {
            let node = group.optical_ref.lock_opm()?;
            let inner_group = node.as_any().downcast_ref::<NodeGroup>().ok_or_else(|| {
                OpossumError::Other("captured group node is not a NodeGroup".into())
            })?;
            inner_group.node_recursive(*member_id)?.0
        };
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| g.add_node_ref(member_ref.clone()))??;
    }
    // `restore_connections` (not `external_connections`) - see the type's doc comment: `external_connections`
    // references the group's own uuid, which no longer exists once `delete_node` above has run.
    for conn in &restore_connections {
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| connect_from_info(g, conn))??;
    }
    Ok(Command::InsertGroup(InsertGroup {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
    }))
}

/// Describes the effect of a [`Command::InsertGroup`] or [`Command::ExtractGroup`] in the GUI-facing
/// [`DocumentChange`] shape.
pub(super) fn describe_group_structure_change(parent_group_id: &Uuid) -> Vec<DocumentChange> {
    vec![DocumentChange::GraphNeedsRefresh {
        graph_id: *parent_group_id,
    }]
}
