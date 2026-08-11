use actix_web::web::{self};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttr, NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    nodes::{ConnectionInfo, NodeGroup},
    opm_document::OpmDocument,
    prelude::{PortType, Proptype},
    types::api_types::{ConnectInfo, NodeInfo},
    utils::LockExt,
};
use std::collections::HashSet;
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

/// Returns whether `candidate_ancestor` is `node_id` itself, or one of `node_id`'s ancestor groups (i.e.
/// `node_id` is nested inside `candidate_ancestor` at any depth).
///
/// # Errors
///
/// Returns an error if `node_id` (or an ancestor of it) doesn't resolve to a node in `scenery`.
fn is_ancestor_or_self(
    scenery: &NodeGroup,
    candidate_ancestor: Uuid,
    node_id: Uuid,
) -> OpmResult<bool> {
    Ok(ancestor_chain(scenery, node_id)?.contains(&candidate_ancestor))
}

/// Returns an error if placing/keeping a reference to `target_id` inside `destination_group_id` would nest
/// the reference inside its own target group, or a group nested within it.
///
/// Analyzing a [`NodeGroup`] holds that group's `Mutex` for the entire duration of its recursive descent
/// into its members. A [`NodeReference`](opossum_core::nodes::NodeReference) nested anywhere inside its
/// own target's subtree would, when analysis reaches it, try to lock that same already-held `Mutex` again
/// on the same thread - a guaranteed self-deadlock. This guards against creating that configuration via
/// direct reference creation, drag-and-drop move, cut, or paste.
///
/// # Errors
///
/// Returns an error if `target_id` is `destination_group_id` itself, or an ancestor of it. Also returns an
/// error if `destination_group_id` (or an ancestor of it) doesn't resolve to a node in `scenery`.
pub fn check_reference_target_not_nested(
    scenery: &NodeGroup,
    target_id: Uuid,
    destination_group_id: Uuid,
) -> OpmResult<()> {
    if is_ancestor_or_self(scenery, target_id, destination_group_id)? {
        return Err(OpossumError::OpticGroup(format!(
            "cannot place a reference to <{target_id}> inside its own target group or a group nested within it"
        )));
    }
    Ok(())
}

/// Returns an error if relocating/pasting `root_ids` - and everything nested inside any of them - into
/// `destination_group_id` would place a `NodeReference` inside its own target group (see
/// [`check_reference_target_not_nested`]).
///
/// A reference whose target is itself part of the expanded `root_ids` set (reference and target being
/// relocated/pasted together) is skipped: they move together as a rigid unit, so their relative structure -
/// and thus this check's outcome - can't change as a result of this particular relocation. An id that
/// doesn't currently resolve in `scenery` is silently skipped, mirroring the existing tolerance for stale
/// ids elsewhere in the relocation/paste machinery.
///
/// # Errors
///
/// Returns an error if a reference among `root_ids` (or nested within one of them) targets
/// `destination_group_id` itself or a group nested within it.
pub fn validate_relocated_references(
    scenery: &NodeGroup,
    root_ids: &[Uuid],
    destination_group_id: Uuid,
) -> OpmResult<()> {
    let mut relocating_ids: Vec<Uuid> = root_ids.to_vec();
    for id in root_ids {
        let Ok((optic_ref, _)) = scenery.node_recursive(*id) else {
            continue;
        };
        let Ok(node) = optic_ref.optical_ref.lock_opm() else {
            continue;
        };
        if let Some(group) = node.as_any().downcast_ref::<NodeGroup>() {
            relocating_ids.extend(group.collect_all_contained_node_ids_recursive()?);
        }
    }
    let relocating_set: HashSet<Uuid> = relocating_ids.iter().copied().collect();

    for id in &relocating_ids {
        let target_id_opt = scenery
            .with_node_attr(*id, |attr| match attr.properties().get("reference id") {
                Ok(Proptype::Uuid(target)) => Some(*target),
                _ => None,
            })
            .ok()
            .flatten();
        if let Some(target_id) = target_id_opt
            && !relocating_set.contains(&target_id)
        {
            check_reference_target_not_nested(scenery, target_id, destination_group_id)?;
        }
    }
    Ok(())
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

/// A node picked out of the document tree by [`collect_nodes_recursive`], together with the group it
/// lives in.
#[derive(Debug)]
pub struct CollectedNode<T> {
    /// UUID of the node itself.
    pub uuid: Uuid,
    /// UUID of the group the node is a direct child of. The recursion knows this anyway, and every
    /// caller that wants to point a user at the node needs it to open the right tab.
    pub group_id: Uuid,
    /// Whatever the selector extracted from the node.
    pub value: T,
}

/// Walk every node below `current_group` - nested subgroups included - and collect what `select`
/// returns for it.
///
/// This is the one recursive document walk the backend uses to answer "which nodes of the whole
/// document are X?". Callers differ only in the `select` closure, so questions like "all source
/// ports" and "all amplifiers" do not each grow their own traversal.
///
/// A subtree that cannot be inspected (e.g. a node that fails to lock) is skipped silently rather
/// than failing the whole walk - a partially readable document should still yield a usable list.
///
/// # Arguments
///
/// * `scenery` - the document's root group.
/// * `current_group` - the group to descend into; pass the root's UUID to cover the whole document.
/// * `select` - returns `Some(value)` for a node that belongs in the result, `None` otherwise.
/// * `collected` - result accumulator, appended to in depth-first order.
pub fn collect_nodes_recursive<T>(
    scenery: &NodeGroup,
    current_group: Uuid,
    select: &impl Fn(&NodeAttr) -> Option<T>,
    collected: &mut Vec<CollectedNode<T>>,
) {
    let children = scenery.with_group_node(current_group, |group| {
        group
            .nodes()
            .iter()
            .map(|node_ref| {
                let node = node_ref.optical_ref.lock_opm()?;
                let node_attr = node.node_attr();
                // Extract everything the caller wants while the lock is held, then release it -
                // the recursion below needs to lock nodes again.
                let selected = (node_attr.uuid(), select(node_attr));
                drop(node);
                Ok(selected)
            })
            .collect::<Result<Vec<(Uuid, Option<T>)>, OpossumError>>()
    });

    let Ok(Ok(children)) = children else {
        return;
    };
    for (child_uuid, selected) in children {
        if let Some(value) = selected {
            collected.push(CollectedNode {
                uuid: child_uuid,
                group_id: current_group,
                value,
            });
        }
        if scenery.with_group_node(child_uuid, |_| {}).is_ok() {
            collect_nodes_recursive(scenery, child_uuid, select, collected);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use opossum_core::nodes::{Dummy, NodeReference};

    /// Builds `root -> g1 -> g2`, returning `(root, g1_id, g2_id)`.
    fn nested_groups() -> (NodeGroup, Uuid, Uuid) {
        let mut root = NodeGroup::new("root");
        let mut g2 = NodeGroup::new("g2");
        g2.add_node(Dummy::default()).unwrap();
        let mut g1 = NodeGroup::new("g1");
        let g2_id = g1.add_node(g2).unwrap();
        let g1_id = root.add_node(g1).unwrap();
        (root, g1_id, g2_id)
    }

    #[test]
    fn is_ancestor_or_self_true_for_self() {
        let (root, g1_id, _) = nested_groups();
        assert!(is_ancestor_or_self(&root, g1_id, g1_id).unwrap());
    }

    #[test]
    fn is_ancestor_or_self_true_for_direct_parent() {
        let (root, g1_id, g2_id) = nested_groups();
        assert!(is_ancestor_or_self(&root, g1_id, g2_id).unwrap());
    }

    #[test]
    fn is_ancestor_or_self_true_for_root_grandparent() {
        let (root, _, g2_id) = nested_groups();
        let root_id = root.node_attr().uuid();
        assert!(is_ancestor_or_self(&root, root_id, g2_id).unwrap());
    }

    #[test]
    fn is_ancestor_or_self_false_for_unrelated_group() {
        let (mut root, _, g2_id) = nested_groups();
        let other_id = root.add_node(NodeGroup::new("other")).unwrap();
        assert!(!is_ancestor_or_self(&root, other_id, g2_id).unwrap());
    }

    #[test]
    fn check_reference_target_not_nested_rejects_self_placement() {
        let (root, g1_id, _) = nested_groups();
        assert!(check_reference_target_not_nested(&root, g1_id, g1_id).is_err());
    }

    #[test]
    fn check_reference_target_not_nested_rejects_nested_descendant() {
        let (root, g1_id, g2_id) = nested_groups();
        assert!(check_reference_target_not_nested(&root, g1_id, g2_id).is_err());
    }

    #[test]
    fn check_reference_target_not_nested_allows_unrelated_group() {
        let (mut root, g1_id, _) = nested_groups();
        let other_id = root.add_node(NodeGroup::new("other")).unwrap();
        assert!(check_reference_target_not_nested(&root, g1_id, other_id).is_ok());
    }

    #[test]
    fn validate_relocated_references_rejects_reference_into_own_target() {
        let mut root = NodeGroup::new("root");
        let g1_id = root.add_node(NodeGroup::new("g1")).unwrap();
        let g1_ref = root.node(g1_id).unwrap();
        let node_reference = NodeReference::from_node(&g1_ref).unwrap();
        let ref_id = root.add_node(node_reference).unwrap();

        assert!(validate_relocated_references(&root, &[ref_id], g1_id).is_err());
    }

    #[test]
    fn validate_relocated_references_rejects_reference_nested_in_moved_group() {
        // root -> T, root -> H { ref(T) }: moving H into T would nest ref(T) inside its own target.
        let mut root = NodeGroup::new("root");
        let t_id = root.add_node(NodeGroup::new("T")).unwrap();
        let t_ref = root.node(t_id).unwrap();
        let node_reference = NodeReference::from_node(&t_ref).unwrap();

        let mut h = NodeGroup::new("H");
        h.add_node(node_reference).unwrap();
        let h_id = root.add_node(h).unwrap();

        assert!(validate_relocated_references(&root, &[h_id], t_id).is_err());
    }

    #[test]
    fn validate_relocated_references_allows_sibling_reference() {
        let mut root = NodeGroup::new("root");
        let a_id = root.add_node(Dummy::default()).unwrap();
        let a_ref = root.node(a_id).unwrap();
        let node_reference = NodeReference::from_node(&a_ref).unwrap();
        let ref_id = root.add_node(node_reference).unwrap();
        let other_id = root.add_node(NodeGroup::new("other")).unwrap();

        assert!(validate_relocated_references(&root, &[ref_id], other_id).is_ok());
    }

    #[test]
    fn validate_relocated_references_skips_co_relocated_target() {
        // Moving a reference and its own target together, as siblings, into an unrelated destination
        // must still be allowed - they keep the same (valid) relative structure either way.
        let mut root = NodeGroup::new("root");
        let g1_id = root.add_node(NodeGroup::new("g1")).unwrap();
        let g1_ref = root.node(g1_id).unwrap();
        let node_reference = NodeReference::from_node(&g1_ref).unwrap();
        let ref_id = root.add_node(node_reference).unwrap();
        let dest_id = root.add_node(NodeGroup::new("dest")).unwrap();

        assert!(validate_relocated_references(&root, &[g1_id, ref_id], dest_id).is_ok());
    }
}
