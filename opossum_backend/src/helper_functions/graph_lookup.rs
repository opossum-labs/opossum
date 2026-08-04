use actix_web::web::{self};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::OpmResult,
    nodes::{ConnectionInfo, NodeGroup},
    opm_document::OpmDocument,
    prelude::{PortType, Proptype},
    types::api_types::{ConnectInfo, NodeInfo},
    utils::LockExt,
};
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

/// Collect the optical references and top-left position of the given nodes.
///
/// Iterates over all provided node UUIDs, resolves their corresponding
/// `OpticRef`s, and determines the minimum `(x, y)` GUI position among them.
/// The returned position can be used as an anchor point for placing a new group.
///
/// # Arguments
///
/// * `nodes_to_convert` - Slice of node UUIDs to collect.
///
/// # Returns
///
/// Returns a tuple containing:
/// - `Vec<OpticRef>`: The resolved optical references of the nodes.
/// - `Point2<f64>`: The minimum `(x, y)` position among all nodes.
///
/// # Notes
///
/// Nodes that cannot be resolved are silently ignored.
#[allow(clippy::significant_drop_tightening)]
pub fn collect_node_refs_and_pos(
    data: &web::Data<AppState>,
    nodes_to_convert: &[Uuid],
) -> (Vec<OpticRef>, Point2<f64>) {
    let document = data.document.lock();
    let scenery = document.scenery();
    let mut corner = Point2::new(f64::INFINITY, f64::INFINITY);
    let optic_ref_vec = nodes_to_convert
        .iter()
        .filter_map(|node| {
            scenery.node_recursive(*node).ok().map(|(r, _)| {
                if let Ok(opt_ref) = r.optical_ref.lock_opm() {
                    // Safely handle cases where a node might not have a GUI position assigned yet
                    if let Some(pos) = opt_ref.gui_position() {
                        corner.x = corner.x.min(pos.x);
                        corner.y = corner.y.min(pos.y);
                    }
                }
                r
            })
        })
        .collect();
    (optic_ref_vec, corner)
}

/// Collect all connections of the given group.
///
/// # Arguments
///
/// * `group_id` - The UUID of the group whose connections should be retrieved.
///
/// # Returns
///
/// Returns a vector of `ConnectionInfo` representing all connections within the group.
///
/// # Errors
///
/// This function will return an error if the `group_id` was not found.
#[allow(clippy::significant_drop_tightening)]
pub fn collect_group_connections(
    data: &web::Data<AppState>,
    group_id: Uuid,
) -> OpmResult<Vec<ConnectionInfo>> {
    let document = data.document.lock();
    let scenery = document.scenery();

    scenery.with_group_node(group_id, opossum_core::nodes::NodeGroup::connections)
}

/// Returns whether `node_id` names a reference node (i.e. one carrying a "reference id" property).
///
/// A node that cannot be resolved counts as "not a reference" rather than erroring - callers only
/// use this to enrich a [`ConnectInfo`]'s `is_reference` flag for the GUI, where a missing target
/// is not worth failing the whole request for.
#[must_use]
pub fn is_reference_target(scenery: &NodeGroup, node_id: Uuid) -> bool {
    scenery
        .with_node_attr(node_id, |attr| {
            attr.properties().get("reference id").is_ok()
        })
        .unwrap_or(false)
}

/// Recursively resolves a "reference" node to the actual node it (transitively) points at, following
/// each `"reference id"` property hop until a non-reference node is reached. Returns the resolved
/// [`OpticRef`] and whether `uuid` itself named a reference node (`false` if `uuid` was already the
/// non-reference target).
///
/// # Errors
///
/// Returns an error if `uuid` doesn't resolve to a node, or a reference node along the chain is
/// missing its `"reference id"` property.
pub fn resolve_reference_chain(
    document: &OpmDocument,
    uuid: Uuid,
) -> Result<(OpticRef, bool), BackEndErrorResponse> {
    let optic_ref = document.scenery().node_recursive(uuid)?.0;
    let node_attr = optic_ref.optical_ref.lock_opm()?.node_attr().clone();
    if node_attr.node_type() == "reference" {
        let Ok(Proptype::Uuid(ref_uuid)) = node_attr.properties().get("reference id") else {
            return Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                "'reference id' property not found",
            ));
        };
        let (resolved, _) = resolve_reference_chain(document, *ref_uuid)?;
        Ok((resolved, true))
    } else {
        Ok((optic_ref, false))
    }
}

/// Captures every connection touching `node_id` within `parent_group_id`'s graph, as `ConnectInfo`s.
/// Used to snapshot a node's wiring before it's deleted, so `Command::AddNode`/`RemoveNode`'s
/// `connections` field can restore it on undo - must be called before the node is actually removed from
/// the graph, since deleting a node silently drops its incident edges.
///
/// # Errors
///
/// This function will return an error if `parent_group_id` doesn't resolve to a group.
pub fn capture_node_connections(
    scenery: &NodeGroup,
    parent_group_id: Uuid,
    node_id: Uuid,
) -> OpmResult<Vec<ConnectInfo>> {
    let connections = scenery.with_group_node(parent_group_id, NodeGroup::connections)?;
    Ok(connections
        .iter()
        .filter(|c| c.src_id == node_id || c.target_id == node_id)
        .map(|c| {
            let is_reference = is_reference_target(scenery, c.target_id);
            ConnectInfo::from_connection_info(c, is_reference)
        })
        .collect())
}

/// Maps `node_id`'s internal port `internal_name` to `external_name` on `g`'s external port map,
/// dispatching on `port_type`. Collapses the `match port_type { Input => map_input_port, Output =>
/// map_output_port }` split that every runtime-`port_type` mapping call would otherwise repeat.
///
/// # Errors
///
/// Returns an error under the same conditions as [`NodeGroup::map_input_port`] /
/// [`NodeGroup::map_output_port`].
pub fn map_port(
    g: &mut NodeGroup,
    port_type: PortType,
    node_id: Uuid,
    internal_name: &str,
    external_name: &str,
) -> OpmResult<()> {
    match port_type {
        PortType::Input => g.map_input_port(node_id, internal_name, external_name),
        PortType::Output => g.map_output_port(node_id, internal_name, external_name),
    }
}

/// Returns `uuid`'s parent group id, or `uuid` itself if it names the scenery root - which has no
/// real parent to report, matching the same self-as-parent sentinel `remove_port_map_cascade` uses
/// for the same reason.
///
/// # Errors
///
/// Returns an error if `uuid` doesn't resolve to a node in `scenery`.
pub fn parent_group_id_or_self(scenery: &NodeGroup, uuid: Uuid) -> OpmResult<Uuid> {
    if uuid == scenery.node_attr().uuid() {
        Ok(uuid)
    } else {
        Ok(scenery.node_recursive(uuid)?.1)
    }
}

/// The chain of group ids from `uuid` up to (and including) the scenery root: `[uuid, parent, ..., root]`.
///
/// # Errors
///
/// Returns an error if `uuid` (or an ancestor of it) doesn't resolve to a node in `scenery`.
fn ancestor_chain(scenery: &NodeGroup, uuid: Uuid) -> OpmResult<Vec<Uuid>> {
    let root = scenery.node_attr().uuid();
    let mut chain = vec![uuid];
    let mut current = uuid;
    while current != root {
        let parent = parent_group_id_or_self(scenery, current)?;
        if parent == current {
            break; // root's self-sentinel (or a node with no distinct parent) - stop.
        }
        chain.push(parent);
        current = parent;
    }
    Ok(chain)
}

/// The lowest common ancestor group of `a` and `b` in `scenery`'s tree.
///
/// This is the tab a move between the two groups was initiated from: an into-group move's outer parent, an
/// out-of-group move's outer parent, or two siblings' shared parent. Used as a move's *direction-stable*
/// focus tab - the change is visible there whichever way the move runs, so undo/redo can stay put instead
/// of being pulled into the group (unlike `MoveNodesRequest::target_group_id`, which flips between undo and
/// redo). Falls back to the scenery root if the two chains share nothing (they always share the root).
///
/// # Errors
///
/// Returns an error if either id (or an ancestor) doesn't resolve to a node in `scenery`.
pub fn lowest_common_ancestor_group(scenery: &NodeGroup, a: Uuid, b: Uuid) -> OpmResult<Uuid> {
    let a_chain = ancestor_chain(scenery, a)?;
    let b_chain = ancestor_chain(scenery, b)?;
    // `a_chain` runs from `a` up to the root; the first of its ids that also appears in `b_chain` is the
    // deepest ancestor the two share.
    Ok(a_chain
        .into_iter()
        .find(|id| b_chain.contains(id))
        .unwrap_or_else(|| scenery.node_attr().uuid()))
}

/// Create a [`NodeInfo`] representation for a newly created group node.
///
/// # Arguments
///
/// * `new_group_id` - The UUID of the new group node.
/// * `pos` - The position where the node should be placed.
///
/// # Returns
///
/// Returns a `NodeInfo` describing the group node, including its ports and position.
///
/// # Errors
///
/// This function will return an error if the node cannot be resolved
/// or if its data cannot be accessed.
#[allow(clippy::significant_drop_tightening)]
pub fn create_new_group_node_info(
    data: &web::Data<AppState>,
    new_group_id: Uuid,
    pos: Point2<f64>,
) -> OpmResult<NodeInfo> {
    let document = data.document.lock();
    let scenery = document.scenery();
    let (new_group_ref, _) = scenery.node_recursive(new_group_id)?;
    let mut new_group_node = new_group_ref.optical_ref.lock_opm()?;
    new_group_node.node_attr_mut().set_gui_position(Some(pos));
    Ok(NodeInfo::from_analyzable(
        &*new_group_node,
        Some(Some((pos.x, pos.y))),
    ))
}
