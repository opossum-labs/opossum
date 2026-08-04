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

use super::{Command, refresh_changes};
use crate::{
    error::BackEndErrorResponse,
    helper_functions::{
        map_port, reconnect_all, relocate_nodes_in_document, remove_relocated_nodes,
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

/// A node move plus the extra groups its reroute touched.
///
/// [`Command::MoveNodes`] carries the [`MoveNodesRequest`] (which already names source and target) plus
/// `affected_groups`: any *other* group whose port map a reroute changed as a side effect (a moved node's
/// pre-existing external mapping can be consumed arbitrarily far out - see `post_move_nodes`). Source and
/// target are always refreshed on undo/redo; `affected_groups` ensures a third tab showing a rerouted
/// export is refreshed too. The set is symmetric across a move and its reverse, so it is carried through
/// `apply` unchanged rather than recomputed.
#[derive(Clone)]
pub struct MoveNodes {
    /// The move itself (source group, target group, nodes).
    pub request: MoveNodesRequest,
    /// Groups beyond source/target whose port map a reroute touched.
    pub affected_groups: Vec<Uuid>,
    /// The tab an undo/redo of this move should focus: the drag's *outer* context (the lowest common
    /// ancestor of source and target). The change is visible there whichever direction the move runs, so
    /// the view stays put instead of being pulled into the group. Unlike `request.target_group_id` (which
    /// flips between a move and its reverse), this is identical for both directions, so it is carried
    /// through `apply` unchanged. See `Command::jump_target`.
    pub focus_group_id: Uuid,
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
    /// Groups (beyond `parent_group_id`) whose port map a reroute touched during this conversion -
    /// refreshed on undo/redo so a tab showing a rerouted export elsewhere isn't left stale. Symmetric
    /// across insert/extract, so carried through unchanged.
    pub affected_groups: Vec<Uuid>,
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
    cmd: MoveNodes,
) -> Result<Command, BackEndErrorResponse> {
    let MoveNodes {
        request,
        affected_groups,
        focus_group_id,
    } = cmd;
    let MoveNodesRequest {
        source_group_id: from_group_id,
        target_group_id: to_group_id,
        nodes_to_move: node_ids,
    } = request;

    // A relocation preserving each node's uuid, re-derived fresh from the live document on every call (not
    // carried in the command) so it stays correct across arbitrary undo/redo cycles - each call only
    // captures what's actually still there to lose at the moment it runs. The forward `post_move_nodes`
    // reports the connection side effects to the GUI; here (undo/redo) the affected tabs are refreshed
    // wholesale via `describe_move_nodes`, so the returned outcome isn't needed.
    relocate_nodes_in_document(document, from_group_id, to_group_id, &node_ids)?;

    // `affected_groups` and `focus_group_id` are the same for this move and its reverse, so carry them
    // through unchanged - that stable focus is what keeps undo and redo landing on the same outer tab.
    Ok(Command::MoveNodes(MoveNodes {
        request: MoveNodesRequest {
            source_group_id: to_group_id,
            target_group_id: from_group_id,
            nodes_to_move: node_ids,
        },
        affected_groups,
        focus_group_id,
    }))
}

/// Describes the effect of a [`Command::MoveNodes`] in the GUI-facing [`DocumentChange`] shape: a
/// `GraphNeedsRefresh` for source, target, and every extra group a reroute touched (`affected_groups`),
/// deduplicated.
pub(super) fn describe_move_nodes(cmd: &MoveNodes) -> Vec<DocumentChange> {
    refresh_changes(
        [cmd.request.source_group_id, cmd.request.target_group_id]
            .into_iter()
            .chain(cmd.affected_groups.iter().copied()),
    )
}

/// Re-points `parent_group_id`'s own external mapping for each of `mappings` at a new internal
/// target, chosen per-mapping by `target` (`apply_insert_group` re-points at the group's own internal
/// port; `apply_extract_group` re-points directly at the member's own port).
fn reroute_mappings(
    document: &mut OpmDocument,
    parent_group_id: Uuid,
    mappings: &[ReroutedMapping],
    target: impl Fn(&ReroutedMapping) -> (Uuid, &str),
) -> Result<(), BackEndErrorResponse> {
    for m in mappings {
        let (target_id, target_port) = target(m);
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| {
                map_port(g, m.port_type, target_id, target_port, &m.external_name)
            })??;
    }
    Ok(())
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
        affected_groups,
    } = cmd;
    // Remove the flat members without cascading references: re-forming the group is a relocation, so an
    // external reference to a member must survive and follow it into the re-created group.
    remove_relocated_nodes(document.scenery_mut(), parent_group_id, &member_ids)?;
    let group_id = group.uuid()?;
    document
        .scenery_mut()
        .with_group_node_mut(parent_group_id, |g| g.add_node_ref(group.clone()))??;
    reconnect_all(document, parent_group_id, &external_connections)?;
    // Re-point `parent_group_id`'s own mapping at the group's own already-correct internal
    // mapping for the same port - the group's internal structure never changes across
    // detach/reattach cycles, so `group_internal_name` is still valid.
    reroute_mappings(document, parent_group_id, &rerouted_mappings, |m| {
        (group_id, m.group_internal_name.as_str())
    })?;
    Ok(Command::ExtractGroup(GroupConversion {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
        rerouted_mappings,
        affected_groups,
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
        affected_groups,
    } = cmd;
    let group_id = group.uuid()?;
    // Remove the group node without cascading references: dissolving it is a relocation of its members
    // back out to the parent, so an external reference to a member must survive. This still strips the
    // parent's own port-map entry pointing at the group (see `remove_node_no_cascade`), which the
    // `rerouted_mappings` re-point below relies on.
    remove_relocated_nodes(document.scenery_mut(), parent_group_id, &[group_id])?;
    for member_id in &member_ids {
        let member_ref = {
            let node = group.optical_ref.lock_opm()?;
            let inner_group = node.as_any().downcast_ref::<NodeGroup>().ok_or_else(|| {
                OpossumError::Other("captured group node is not a NodeGroup".into())
            })?;
            let result = inner_group.node_recursive(*member_id)?.0;
            drop(node);
            result
        };
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| g.add_node_ref(member_ref.clone()))??;
    }
    // `restore_connections` (not `external_connections`) - see the type's doc comment: `external_connections`
    // references the group's own uuid, which no longer exists once `delete_node` above has run.
    reconnect_all(document, parent_group_id, &restore_connections)?;
    // Re-point `parent_group_id`'s own mapping directly at the member's own port - the old entry
    // pointing at the (now-deleted) group is already gone, stripped by this function's own
    // `delete_node(group_id)` call above.
    reroute_mappings(document, parent_group_id, &rerouted_mappings, |m| {
        (m.member_id, m.member_port.as_str())
    })?;
    Ok(Command::InsertGroup(GroupConversion {
        parent_group_id,
        group,
        member_ids,
        external_connections,
        restore_connections,
        rerouted_mappings,
        affected_groups,
    }))
}

/// Describes the effect of a [`Command::InsertGroup`] or [`Command::ExtractGroup`] in the GUI-facing
/// [`DocumentChange`] shape: a `GraphNeedsRefresh` for `parent_group_id` and every extra group a
/// reroute touched (`affected_groups`), deduplicated.
///
/// `dissolved_group` names the group node this change *deletes* (set by `ExtractGroup`, `None` for
/// `InsertGroup` which re-creates it). It is excluded from the refresh set - refreshing a now-gone
/// group would make the GUI fetch a uuid that no longer exists and 400 - and instead gets a
/// `GraphClosed` so the GUI closes its (now-orphaned) tab.
pub(super) fn describe_group_structure_change(
    parent_group_id: &Uuid,
    affected_groups: &[Uuid],
    dissolved_group: Option<Uuid>,
) -> Vec<DocumentChange> {
    let ids = std::iter::once(*parent_group_id)
        .chain(affected_groups.iter().copied())
        .filter(|graph_id| dissolved_group != Some(*graph_id));
    let mut changes = refresh_changes(ids);
    if let Some(graph_id) = dissolved_group {
        changes.push(DocumentChange::GraphClosed { graph_id });
    }
    changes
}

#[cfg(test)]
mod test {
    use super::{
        DocumentChange, MoveNodes, MoveNodesRequest, describe_group_structure_change,
        describe_move_nodes,
    };
    use uuid::Uuid;

    fn refreshed_graph_ids(changes: &[DocumentChange]) -> Vec<Uuid> {
        changes
            .iter()
            .filter_map(|c| match c {
                DocumentChange::GraphNeedsRefresh { graph_id, .. } => Some(*graph_id),
                _ => None,
            })
            .collect()
    }

    /// Regression test for the gap where undoing a move only refreshed source and target, leaving a
    /// third group whose port map a reroute touched visually stale. `describe_move_nodes` must now emit
    /// a `GraphNeedsRefresh` for source, target, and every `affected_group`.
    #[test]
    fn describe_move_nodes_refreshes_source_target_and_affected_groups() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let third = Uuid::new_v4();
        let cmd = MoveNodes {
            request: MoveNodesRequest {
                source_group_id: source,
                target_group_id: target,
                nodes_to_move: vec![Uuid::new_v4()],
            },
            affected_groups: vec![third],
            focus_group_id: source,
        };
        let refreshed = refreshed_graph_ids(&describe_move_nodes(&cmd));
        assert!(refreshed.contains(&source), "source tab must be refreshed");
        assert!(refreshed.contains(&target), "target tab must be refreshed");
        assert!(
            refreshed.contains(&third),
            "a third group a reroute touched must also be refreshed on undo/redo"
        );
    }

    /// Regression test for the same gap on the group-conversion path: undoing a convert/ungroup must
    /// refresh the parent tab *and* every extra group a reroute touched.
    #[test]
    fn describe_group_structure_change_refreshes_parent_and_affected_groups() {
        let parent = Uuid::new_v4();
        let third = Uuid::new_v4();
        let refreshed =
            refreshed_graph_ids(&describe_group_structure_change(&parent, &[third], None));
        assert!(refreshed.contains(&parent), "parent tab must be refreshed");
        assert!(
            refreshed.contains(&third),
            "a third group a reroute touched must also be refreshed on undo/redo"
        );
    }

    /// Regression test for the three `400 node with given uuid does not exist` logs on undoing a
    /// convert-to-group: `ExtractGroup` deletes the group node, so `describe` must exclude that
    /// group's own uuid from the refresh set even though it rode along in `affected_groups` (pushed
    /// there by the forward conversion so a redo would refresh it). Excluding it stops the GUI from
    /// refreshing a tab whose group no longer exists.
    #[test]
    fn describe_group_structure_change_excludes_the_deleted_group() {
        let parent = Uuid::new_v4();
        let deleted_group = Uuid::new_v4();
        // As on `ExtractGroup`: the deleted group's own uuid is in `affected_groups`.
        let refreshed = refreshed_graph_ids(&describe_group_structure_change(
            &parent,
            &[deleted_group],
            Some(deleted_group),
        ));
        assert!(refreshed.contains(&parent), "parent tab must still refresh");
        assert!(
            !refreshed.contains(&deleted_group),
            "the group that extract just deleted must not be refreshed"
        );
        // Without an exclusion (the `InsertGroup`/redo direction) it is refreshed.
        let refreshed_insert = refreshed_graph_ids(&describe_group_structure_change(
            &parent,
            &[deleted_group],
            None,
        ));
        assert!(
            refreshed_insert.contains(&deleted_group),
            "inserting (re)creates the group, so its tab is refreshed"
        );
    }

    /// Undoing a convert-to-group (`ExtractGroup`) must also tell the GUI to close the dissolved
    /// group's own tab (`GraphClosed`), else it stays open showing a group that no longer exists.
    /// `InsertGroup` (redo, which re-creates the group) must NOT close it.
    #[test]
    fn describe_group_structure_change_closes_the_dissolved_group_tab() {
        let parent = Uuid::new_v4();
        let dissolved = Uuid::new_v4();

        let extract = describe_group_structure_change(&parent, &[dissolved], Some(dissolved));
        assert!(
            extract.iter().any(
                |c| matches!(c, DocumentChange::GraphClosed { graph_id } if *graph_id == dissolved)
            ),
            "extracting must emit GraphClosed for the dissolved group, got {extract:?}"
        );

        let insert = describe_group_structure_change(&parent, &[dissolved], None);
        assert!(
            !insert
                .iter()
                .any(|c| matches!(c, DocumentChange::GraphClosed { .. })),
            "inserting must not close any tab, got {insert:?}"
        );
    }
}
