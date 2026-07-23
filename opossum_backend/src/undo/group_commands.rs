//! `apply`/`describe` bodies for the group-structure [`Command`] variants: [`Command::MoveNodes`],
//! [`Command::InsertGroup`], [`Command::ExtractGroup`].
use opossum_core::{
    core_optics::OpticRef,
    error::OpossumError,
    nodes::NodeGroup,
    opm_document::OpmDocument,
    prelude::PortType,
    types::api_types::{ConnectInfo, DocumentChange, MoveNodesRequest},
    utils::LockExt,
};
use uuid::Uuid;

use super::Command;
use crate::{
    error::BackEndErrorResponse,
    helper_functions::{
        connect_from_info, disconnect_moved_node_connections, reconnect_moved_node_connections,
        split_sort_connections_from_document,
    },
};

/// A port-map-only export (no live connection at this level) that was rerouted through the group
/// instead of a live edge: `parent_group_id`'s own `external_name` used to map directly to
/// `member_id`'s `member_port`, and now instead resolves through the group's own internal mapping
/// `group_internal_name -> (member_id, member_port)`. Neither `external_connections` nor
/// `restore_connections` (both live-edge shapes) can represent this - it's the group-conversion
/// analogue of `disconnect_moved_node_connections`/`reconnect_moved_node_connections`'s own
/// `from_group_external_name` reroute case, just needing to survive across undo/redo instead of
/// being applied once.
#[derive(Clone)]
pub struct ReroutedMapping {
    /// `parent_group_id`'s own external name - constant across undo/redo, never changes.
    pub external_name: String,
    pub port_type: PortType,
    /// The member node's original uuid - stable across undo/redo since it's the same live `OpticRef`.
    pub member_id: Uuid,
    /// The member's own internal port name.
    pub member_port: String,
    /// The group's own external name for the same port, on its own untouched internal port map -
    /// baked in once when the mapping was first rerouted, and reused unchanged on every redo since
    /// the group's internal structure never changes across detach/reattach cycles.
    pub group_internal_name: String,
}

/// A captured group/flat-members pair, ready to be converted in either direction.
///
/// Used by both [`Command::InsertGroup`] (removes `member_ids` from `parent_group_id`'s flat graph and
/// inserts `group` in their place, reconnecting `external_connections` - in terms of the *group's own*
/// uuid/exposed-port-names - to its exposed ports, and re-establishing `rerouted_mappings` on
/// `parent_group_id`'s own port map) and [`Command::ExtractGroup`] (the inverse: removes `group` from
/// `parent_group_id` and re-inserts its members as flat nodes in its place, reconnecting
/// `restore_connections` - every connection that touched a member *before* grouping, expressed in terms
/// of the original member uuids/ports, since `external_connections` references the group's own uuid and
/// can't be used once the group node has just been deleted - and re-establishing `rerouted_mappings`
/// directly onto each member's own port). The two are each other's inverse, so they carry exactly the
/// same data; the group's internal members/connections are untouched by either direction - they live
/// inside `group`'s own nested graph the whole time, whether or not `group` is currently attached to the
/// document.
#[derive(Clone)]
pub struct GroupConversion {
    /// The group both `group` and its flat `member_ids` attach to/detach from.
    pub parent_group_id: Uuid,
    /// The group node itself.
    pub group: OpticRef,
    /// The group's direct members, as flat node uuids in `parent_group_id`'s graph.
    pub member_ids: Vec<Uuid>,
    /// Connections to `group`'s own exposed ports, in terms of the group's uuid/port names.
    pub external_connections: Vec<ConnectInfo>,
    /// Connections to a member's own port, in terms of that member's uuid/port name.
    pub restore_connections: Vec<ConnectInfo>,
    /// Port-map-only exports rerouted through the group instead of a live edge.
    pub rerouted_mappings: Vec<ReroutedMapping>,
}

/// Moves `request.nodes_to_move` (and the connections purely between them) from
/// `request.source_group_id` to `request.target_group_id`, returning the swapped [`Command::MoveNodes`]
/// that undoes it. Any connection that can't directly follow the move (a boundary sibling left behind, or
/// a pre-existing external mapping of a moved node's own port) is preserved via
/// `preserve_moved_node_connections` rather than disconnected - see its own docs for why that makes the
/// returned inverse a plain swapped `MoveNodes`, with no extra captured state needed: re-running this same
/// discovery from live state on the next call (whichever direction that is) is enough to correctly unwind
/// whatever this call set up.
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

    // Re-derived fresh from the live document on every call (not carried in the command), so this stays
    // correct across arbitrary undo/redo cycles - each call only captures what's actually still there to
    // lose at the moment it runs.
    let connections = document
        .scenery()
        .with_group_node(from_group_id, NodeGroup::connections)?;
    let split = split_sort_connections_from_document(document, &connections, &node_ids);
    let boundary_connections: Vec<ConnectInfo> =
        split.input.into_iter().chain(split.output).collect();

    // Tear down anything that would otherwise be lost by the move, before the nodes are actually deleted
    // from `from_group_id`. What's captured here can only be re-established once the nodes actually exist
    // in `to_group_id` (see `disconnect_moved_node_connections`'s own docs), so that happens further down.
    let (pending, _removed_connections) = disconnect_moved_node_connections(
        document.scenery_mut(),
        from_group_id,
        to_group_id,
        &boundary_connections,
        &node_ids,
    )?;

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
    for conn in &split.inside {
        document
            .scenery_mut()
            .with_group_node_mut(to_group_id, |g| connect_from_info(g, conn))??;
    }

    reconnect_moved_node_connections(document.scenery_mut(), from_group_id, to_group_id, pending)?;

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
    cmd: GroupConversion,
) -> Result<Command, BackEndErrorResponse> {
    let GroupConversion {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
        rerouted_mappings,
    } = cmd;
    for member_id in &member_ids {
        document.scenery_mut().delete_node(*member_id)?;
    }
    let group_id = group.uuid()?;
    document
        .scenery_mut()
        .with_group_node_mut(parent_group_id, |g| g.add_node_ref(group.clone()))??;
    for conn in &external_connections {
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| connect_from_info(g, conn))??;
    }
    // Re-point `parent_group_id`'s own mapping at the group's own already-correct internal
    // mapping for the same port - the group's internal structure never changes across
    // detach/reattach cycles, so `group_internal_name` is still valid.
    for m in &rerouted_mappings {
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| match m.port_type {
                PortType::Input => {
                    g.map_input_port(group_id, &m.group_internal_name, &m.external_name)
                }
                PortType::Output => {
                    g.map_output_port(group_id, &m.group_internal_name, &m.external_name)
                }
            })??;
    }
    Ok(Command::ExtractGroup(GroupConversion {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
        rerouted_mappings,
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
    cmd: GroupConversion,
) -> Result<Command, BackEndErrorResponse> {
    let GroupConversion {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
        rerouted_mappings,
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
    // Re-point `parent_group_id`'s own mapping directly at the member's own port - the old entry
    // pointing at the (now-deleted) group is already gone, stripped by this function's own
    // `delete_node(group_id)` call above.
    for m in &rerouted_mappings {
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| match m.port_type {
                PortType::Input => g.map_input_port(m.member_id, &m.member_port, &m.external_name),
                PortType::Output => {
                    g.map_output_port(m.member_id, &m.member_port, &m.external_name)
                }
            })??;
    }
    Ok(Command::InsertGroup(GroupConversion {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
        rerouted_mappings,
    }))
}

/// Describes the effect of a [`Command::InsertGroup`] or [`Command::ExtractGroup`] in the GUI-facing
/// [`DocumentChange`] shape.
pub(super) fn describe_group_structure_change(parent_group_id: &Uuid) -> Vec<DocumentChange> {
    vec![DocumentChange::GraphNeedsRefresh {
        graph_id: *parent_group_id,
    }]
}
