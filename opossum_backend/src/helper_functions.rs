use std::{collections::HashSet, pin::Pin};

use actix_web::{
    FromRequest, HttpRequest, HttpResponse,
    dev::Payload,
    web::{self},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    meter,
    nodes::{ConnectionInfo, NodeGroup},
    opm_document::{AnalyzerInfo, OpmDocument},
    prelude::{PortMap, PortType},
    types::api_types::{ConnectInfo, NodeInfo},
    utils::LockExt,
};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

type CascadeRemovalResult = (
    Vec<(Uuid, ConnectInfo)>,
    Vec<(Uuid, Uuid, String, PortType)>,
);

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

/// Looks up the analyzer with the given `id` in `document`, mutably.
///
/// # Errors
///
/// Returns a 404 ([`BackEndErrorResponse::analyzer_not_found`]) if `id` doesn't resolve to an
/// analyzer.
pub fn analyzer_mut_or_404(
    document: &mut OpmDocument,
    id: Uuid,
) -> Result<&mut AnalyzerInfo, BackEndErrorResponse> {
    document
        .analyzer_mut(id)
        .ok_or_else(BackEndErrorResponse::analyzer_not_found)
}

/// Removes every node in `node_ids` from `source_group_id` **without** cascading to reference nodes.
///
/// A group / move / convert *relocates* its nodes: each keeps its uuid and is immediately re-added
/// elsewhere. So - unlike a real delete - a [`NodeReference`](opossum_core::prelude::NodeReference)
/// pointing at a moved node must survive and keep resolving to it in its new location. This removes only
/// the named nodes (direct members of `source_group_id`) via [`NodeGroup::remove_node_no_cascade`], leaving
/// any referrer (inside or outside the moved set) untouched; the reference's `Weak` keeps pointing at the
/// same still-alive `Arc`. It also handles the moved set containing a node *and* a reference to it (both
/// are removed independently and re-added), which the previous cascade-plus-dedup logic existed to work
/// around. Shared by the forward convert / move (`post_convert_nodes_to_group` / `post_move_nodes`) and
/// their undo/redo (`apply_move_nodes`, `apply_insert_group`, `apply_extract_group`).
///
/// # Errors
///
/// Returns an error if `source_group_id` doesn't resolve to a group, or an id isn't a member of it.
pub fn remove_relocated_nodes(
    scenery: &mut NodeGroup,
    source_group_id: Uuid,
    node_ids: &[Uuid],
) -> OpmResult<()> {
    for id in node_ids {
        scenery.with_group_node_mut(source_group_id, |g| g.remove_node_no_cascade(*id))??;
    }
    Ok(())
}

/// Everything a single-group relocation ([`relocate_nodes_in_document`]) changed: the GUI-facing
/// connection side effects plus what was re-established to keep links alive across the move. Mirrors the
/// fields of [`MoveNodesResponse`](opossum_core::types::api_types::MoveNodesResponse), which every caller
/// ultimately reports (directly, or reused by the cut operation).
pub struct RelocationOutcome {
    /// Connections torn down as a side effect of the move, paired with the group each lived in - always
    /// alongside a matching `preserved.new_connections` entry that restores the same logical link.
    pub removed_connections: Vec<(Uuid, ConnectInfo)>,
    /// Connections/mappings re-established to keep links alive across the move (see [`PreservedConnections`]).
    pub preserved: PreservedConnections,
}

/// Relocates `node_ids` from `from_group_id` to `to_group_id` in an already-locked document, **preserving
/// each node's uuid** (a move, not a copy). Boundary connections and pre-existing port mappings are
/// rerouted rather than lost, references to a moved node survive (non-cascading removal via
/// [`remove_relocated_nodes`]), and connections purely between the moved nodes are re-established in the
/// destination. The moved nodes are the same live [`OpticRef`]s, so their whole internal subtree (for a
/// group) travels along and every reference/port-map/connection keyed on their uuids stays valid with no
/// remapping.
///
/// This is the shared core of the forward move (`post_move_nodes`), its undo/redo (`apply_move_nodes`),
/// and the cut operation - all three relocate a set of nodes between two groups in exactly this way.
///
/// # Errors
///
/// Returns an error if either group id doesn't resolve, or a moved node's uuid can't be found.
pub fn relocate_nodes_in_document(
    document: &mut OpmDocument,
    from_group_id: Uuid,
    to_group_id: Uuid,
    node_ids: &[Uuid],
) -> OpmResult<RelocationOutcome> {
    let connections = document
        .scenery()
        .with_group_node(from_group_id, NodeGroup::connections)?;
    let split = split_sort_connections_from_document(document, &connections, node_ids);
    let boundary_connections: Vec<ConnectInfo> =
        split.input.into_iter().chain(split.output).collect();

    // Tear down anything the move would otherwise lose, before the nodes are removed from `from_group_id`
    // (this inspects what's currently mapped/connected there). What's captured here can only be
    // re-established once the nodes actually exist in `to_group_id`, so that happens further down.
    let (pending, removed_connections) = disconnect_moved_node_connections(
        document.scenery_mut(),
        from_group_id,
        to_group_id,
        &boundary_connections,
        node_ids,
    )?;

    let node_refs: Vec<OpticRef> = node_ids
        .iter()
        .filter_map(|id| document.scenery().node_recursive(*id).ok().map(|(r, _)| r))
        .collect();

    // Remove without cascading references (a move is a relocation - an external reference to a moved node
    // must survive; a reference inside the moved set is simply removed and re-added like any other member).
    remove_relocated_nodes(document.scenery_mut(), from_group_id, node_ids)?;
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

    let preserved = reconnect_moved_node_connections(
        document.scenery_mut(),
        from_group_id,
        to_group_id,
        pending,
    )?;

    Ok(RelocationOutcome {
        removed_connections,
        preserved,
    })
}

/// Everything [`sever_external_links`] tore down for a set of co-parented nodes: the direct external
/// connections dropped and the outward-exposed port-map chains cascade-removed - plus, unlike
/// [`CutRelocationOutcome`], the internal connections it deliberately left alone, for a relocating caller
/// to re-create at the destination.
pub struct SeveredLinksOutcome {
    /// Every direct connection in `group_id`'s graph between a member of `node_ids` and a node outside
    /// it, paired with `group_id`. Reported to the GUI and restored on undo (one
    /// [`Command::AddEdge`](crate::undo::Command::AddEdge) each).
    pub removed_connections: Vec<(Uuid, ConnectInfo)>,
    /// The outward port-map chains torn down - flattened for the GUI response via
    /// [`split_cascades_for_response`] and turned into the undo restore via
    /// `Command::from(&PortMapCascadeRemoval)`.
    pub cascades: Vec<PortMapCascadeRemoval>,
    /// Groups whose port maps changed (every cascade level's group plus `group_id` itself), so the GUI
    /// re-fetches their exposed ports.
    pub port_map_groups_changed: Vec<Uuid>,
    /// Connections in `group_id`'s graph between two members of `node_ids` - left untouched by this
    /// function; a caller that relocates the nodes elsewhere must re-create these at the destination to
    /// keep them alive.
    pub inside: Vec<ConnectInfo>,
}

/// Severs every link a set of co-parented nodes has to something *outside* that set, within
/// `group_id`'s graph - the shared teardown both branches of a cut use, whether or not the nodes end up
/// relocating. Connections between two `node_ids` members are deliberately left alone (see
/// [`SeveredLinksOutcome::inside`]); only connections crossing the `node_ids` boundary are severed.
///
/// Classifies `group_id`'s connections via [`split_sort_connections_from_document`], then:
/// - disconnects every boundary (`input`/`output`) connection via [`NodeGroup::disconnect_nodes`],
///   capturing each one (for the GUI response + undo) before it's gone;
/// - cascade-removes each node's outward-exposed port-map chain via
///   [`disconnect_exposed_port_cascades_for_node`] (a port-map chain, by construction, always terminates
///   outside `group_id`, so it's always an external link, never one to another `node_ids` member).
///
/// # Arguments
///
/// * `document` - the already-locked live document.
/// * `group_id` - the group the nodes currently live in.
/// * `node_ids` - the nodes being cut together.
///
/// # Returns
///
/// A [`SeveredLinksOutcome`] with the severed connections, port-map cascades, the groups whose port maps
/// changed, and the internal connections preserved for the caller.
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group, or a disconnect/cascade step fails.
pub fn sever_external_links(
    document: &mut OpmDocument,
    group_id: Uuid,
    node_ids: &[Uuid],
) -> OpmResult<SeveredLinksOutcome> {
    let connections = document
        .scenery()
        .with_group_node(group_id, NodeGroup::connections)?;
    let split = split_sort_connections_from_document(document, &connections, node_ids);

    let mut removed_connections = Vec::new();
    for conn in split.input.iter().chain(&split.output) {
        document.scenery_mut().with_group_node_mut(group_id, |g| {
            g.disconnect_nodes(conn.src_uuid(), conn.src_port())
        })??;
        removed_connections.push((group_id, conn.clone()));
    }

    // Cascade-tear-down each node's outward-exposed port-map chains (removes `group_id`'s own entry,
    // walks through any re-exporting ancestor, and disconnects the terminal edge).
    let mut cascades = Vec::new();
    for id in node_ids {
        cascades.extend(disconnect_exposed_port_cascades_for_node(
            document.scenery_mut(),
            group_id,
            *id,
        )?);
    }

    let mut port_map_groups_changed: Vec<Uuid> = cascades
        .iter()
        .flat_map(|c| c.levels.iter().map(|l| l.group_id))
        .collect();
    port_map_groups_changed.push(group_id);
    port_map_groups_changed.sort();
    port_map_groups_changed.dedup();

    Ok(SeveredLinksOutcome {
        removed_connections,
        cascades,
        port_map_groups_changed,
        inside: split.inside,
    })
}

/// Everything a cut relocation ([`relocate_nodes_severing_external_links`]) tore down, so the caller can
/// both report it to the GUI and build the undo step that restores it.
pub struct CutRelocationOutcome {
    /// Every direct connection in `from_group_id`'s graph between a moved node and a node left behind,
    /// paired with `from_group_id`. Dropped when the nodes are removed; reported to the GUI and restored
    /// on undo (one [`Command::AddEdge`](crate::undo::Command::AddEdge) each).
    pub removed_connections: Vec<(Uuid, ConnectInfo)>,
    /// The outward port-map chains torn down - flattened for the GUI response via
    /// [`split_cascades_for_response`] and turned into the undo restore via
    /// `Command::from(&PortMapCascadeRemoval)`.
    pub cascades: Vec<PortMapCascadeRemoval>,
    /// Groups whose port maps changed (every cascade level's group plus `from_group_id`), so the GUI
    /// re-fetches their exposed ports.
    pub port_map_groups_changed: Vec<Uuid>,
}

/// Relocates `node_ids` from `from_group_id` to `to_group_id` **preserving each node's uuid**. Unlike
/// [`relocate_nodes_in_document`], boundary connections (to a node left behind) are **severed** rather
/// than rerouted through a new port mapping - this is the cut operation's relocation: a cut node keeps
/// its identity (so a [`NodeReference`](opossum_core::prelude::NodeReference) pointing at it stays valid)
/// but arrives at `to_group_id` with only the connections it had to other members of `node_ids` -
/// connections to anything left behind are dropped, exactly as the old duplicate-based cut left the
/// pasted copy after `delete_node`ing the original. Connections between two co-moved nodes, however, are
/// preserved - re-created in `to_group_id` once both ends have landed there.
///
/// Severs everything crossing the `node_ids` boundary via [`sever_external_links`] (which also captures
/// the internal connections to preserve); [`remove_relocated_nodes`] then removes the nodes without
/// cascading references (dropping their now-bare remaining state), each same [`OpticRef`] is re-added to
/// `to_group_id`, and finally each preserved internal connection is re-created there via
/// [`connect_from_info`]. Because it never calls the `disconnect_moved_node_connections` /
/// `reconnect_moved_node_connections` reroute machinery, none of the plain move's boundary/mapping
/// rerouting special cases can fire.
///
/// # Arguments
///
/// * `document` - the already-locked live document.
/// * `from_group_id` - the group the nodes currently live in.
/// * `to_group_id` - the group to move them into.
/// * `node_ids` - the nodes to relocate.
///
/// # Returns
///
/// A [`CutRelocationOutcome`] with the torn-down connections, port-map cascades, and the set of groups
/// whose port maps changed.
///
/// # Errors
///
/// Returns an error if either group id doesn't resolve, a moved node's uuid can't be found, or a
/// severing / connection step fails.
pub fn relocate_nodes_severing_external_links(
    document: &mut OpmDocument,
    from_group_id: Uuid,
    to_group_id: Uuid,
    node_ids: &[Uuid],
) -> OpmResult<CutRelocationOutcome> {
    let SeveredLinksOutcome {
        removed_connections,
        cascades,
        port_map_groups_changed,
        inside,
    } = sever_external_links(document, from_group_id, node_ids)?;

    // Relocate the now-boundary-free nodes: same live `OpticRef`s (uuid preserved, references survive),
    // removed without cascading, re-added to the destination.
    let node_refs: Vec<OpticRef> = node_ids
        .iter()
        .filter_map(|id| document.scenery().node_recursive(*id).ok().map(|(r, _)| r))
        .collect();
    remove_relocated_nodes(document.scenery_mut(), from_group_id, node_ids)?;
    for node_ref in &node_refs {
        document
            .scenery_mut()
            .with_group_node_mut(to_group_id, |g| g.add_node_ref(node_ref.clone()))??;
    }

    // Connections purely between moved nodes survive the move - re-create them now that both ends live
    // in `to_group_id` (their own edges were dropped along with the rest of each node's state when it
    // was removed above).
    for conn in &inside {
        document
            .scenery_mut()
            .with_group_node_mut(to_group_id, |g| connect_from_info(g, conn))??;
    }

    Ok(CutRelocationOutcome {
        removed_connections,
        cascades,
        port_map_groups_changed,
    })
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

/// Before `node_id` (living in `parent_group_id`) is deleted, tears down every port-map chain that
/// exposes one of its ports outward, cascading through *all* re-exporting groups - not just one hop.
///
/// If a node's port is exposed on its own group `G`, re-exposed one level further out on `G`'s parent,
/// and so on until some ancestor holds the live connection that ultimately consumes it, deleting the
/// node must remove every one of those chained port-map entries and disconnect that terminal
/// connection. A single-hop teardown (which this replaces) left the outer groups' mappings and the
/// terminal edge dangling for a doubly-nested node. Delegates the actual outward walk to
/// [`remove_port_map_cascade`] (the same one the remove-port-map endpoint uses), invoked once per port
/// `node_id` exposes on `parent_group_id`.
///
/// Unlike the previous single-hop helper, this *does* remove the port-map entries as it goes (that is
/// what the cascade requires); the caller's subsequent `NodeGroup::delete_node` then finds the
/// innermost entry already gone and only prunes anything the cascade didn't reach.
///
/// `scenery` must be the document root (`document.scenery_mut()`). If `parent_group_id` is the root
/// itself, a node there exposes nothing outward, so this returns `Ok(vec![])`.
///
/// # Errors
///
/// Returns an error if `parent_group_id` doesn't resolve to a group, or a cascade step fails.
pub fn disconnect_exposed_port_cascades_for_node(
    scenery: &mut NodeGroup,
    parent_group_id: Uuid,
    node_id: Uuid,
) -> OpmResult<Vec<PortMapCascadeRemoval>> {
    if parent_group_id == scenery.node_attr().uuid() {
        return Ok(Vec::new());
    }

    let mapped: Vec<(PortType, String)> = scenery.with_group_node(parent_group_id, |g| {
        [PortType::Input, PortType::Output]
            .into_iter()
            .flat_map(|port_type| {
                g.graph()
                    .port_map(&port_type)
                    .assigned_ports_for_node(node_id)
                    .into_iter()
                    .map(move |(external_name, _internal_name)| (port_type, external_name))
            })
            .collect::<Vec<_>>()
    })?;

    let mut cascades = Vec::new();
    for (port_type, external_name) in mapped {
        if let Some(cascade) =
            remove_port_map_cascade(scenery, parent_group_id, &external_name, port_type)?
        {
            cascades.push(cascade);
        }
    }
    Ok(cascades)
}

/// Flattens the cascades torn down by [`disconnect_exposed_port_cascades_for_node`] into the two
/// response shapes the GUI consumes: every external connection disconnected (paired with the group
/// whose graph held it) and every port-map entry removed `(group_id, internal_node_id,
/// external_port_name, port_type)` across all cascade levels. Same shapes as `DeleteNodeResponse`'s
/// fields; the GUI applies each removal per `group_id`, so a multi-level cascade updates every
/// affected group's tab.
#[must_use]
pub fn split_cascades_for_response(cascades: &[PortMapCascadeRemoval]) -> CascadeRemovalResult {
    let mut disconnected_connections = Vec::new();
    let mut removed_port_mappings = Vec::new();
    for cascade in cascades {
        disconnected_connections.extend(cascade.disconnected_connections.iter().cloned());
        for level in &cascade.levels {
            removed_port_mappings.push((
                level.group_id,
                level.internal_node_id,
                level.external_port_name.clone(),
                level.port_type,
            ));
        }
    }
    (disconnected_connections, removed_port_mappings)
}

/// One level of a cascading port-map removal - one group's own mapping entry that was removed,
/// with enough captured to recreate it on undo.
pub struct RemovedPortMapLevel {
    /// The group whose own port-map entry was removed at this level.
    pub group_id: Uuid,
    /// `group_id`'s own parent (where `group_id`'s exposed port is rendered/connected).
    pub parent_group_id: Uuid,
    /// The external name `group_id` exposed the port under.
    pub external_port_name: String,
    /// The node the removed entry pointed at - a plain node at the innermost level, the next-inner
    /// group at every re-exporting level further out.
    pub internal_node_id: Uuid,
    /// The port name on `internal_node_id` the removed entry pointed at.
    pub internal_port_name: String,
    /// Whether the mapping exposed an input or an output port.
    pub port_type: PortType,
}

/// Everything torn down by [`remove_port_map_cascade`], innermost level first.
pub struct PortMapCascadeRemoval {
    pub levels: Vec<RemovedPortMapLevel>,
    /// The live connection(s) finally torn down where the cascade terminated, paired with the
    /// group whose graph held them - empty if the chain was already orphaned (nothing consuming
    /// it at the top).
    pub disconnected_connections: Vec<(Uuid, ConnectInfo)>,
}

/// Removes `group_id`'s own port-map entry for `external_port_name`, then walks outward: if
/// `group_id`'s immediate parent has a live connection consuming it, disconnects that connection
/// and stops - the cascade is complete. Otherwise, if `group_id` is itself re-exposed one level
/// further out (the parent's own port map has an entry pointing at `(group_id,
/// external_port_name)`), removes that entry too and continues the walk from there. Stops when a
/// live connection is found and disconnected, or when neither a live connection nor a further
/// chained mapping exists at the current level (an orphaned/unused mapping chain).
///
/// Conceptually, the node this chain ultimately exposes "is" connected to whatever the chain
/// terminates at - the intermediate port maps are just bookkeeping for the nesting structure, so
/// removing the innermost mapping takes the whole chain down with it rather than leaving the rest
/// dangling.
///
/// Always terminates and never revisits a group: containment is a strict tree, and each step
/// moves to `group_id`'s own parent, strictly closer to the root. Fan-out (more than one live
/// connection at the terminating level) can't happen by construction - `connect_nodes` refuses a
/// second connection on an already-connected port, and `map_port` refuses to map an
/// already-connected one - so "a live edge exists here" and "this is chained further out" are
/// mutually exclusive at any given level; this still collects into a `Vec` rather than assuming
/// exactly one, so it degrades gracefully rather than panicking if that invariant ever changes.
///
/// `scenery` must be the document root.
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group, or if disconnecting a found
/// connection fails.
///
/// # Returns
///
/// `Ok(None)` if `group_id` has no mapping named `external_port_name` of that `port_type` at all.
pub fn remove_port_map_cascade(
    scenery: &mut NodeGroup,
    group_id: Uuid,
    external_port_name: &str,
    port_type: PortType,
) -> OpmResult<Option<PortMapCascadeRemoval>> {
    let mut levels = Vec::new();
    let mut disconnected_connections = Vec::new();
    let mut cur_group = group_id;
    let mut cur_name = external_port_name.to_string();

    loop {
        let Some((internal_node_id, internal_port_name)) =
            scenery.with_group_node_mut(cur_group, |g| {
                let hit = g.graph().port_map(&port_type).get(&cur_name).cloned();
                if hit.is_some() {
                    g.remove_mapped_port(&cur_name, port_type);
                }
                hit
            })?
        else {
            if levels.is_empty() {
                return Ok(None);
            }
            // Only reachable if the invariant established by the `Some(outer_name)` branch below
            // (the parent's port map contains this exact entry, checked under the same `&mut`
            // scenery reference with no concurrent mutation possible in between) were somehow
            // violated - defensive only, not a real code path.
            return Err(OpossumError::Other(
                "chained port mapping vanished mid-cascade".into(),
            ));
        };

        let root_id = scenery.node_attr().uuid();
        if cur_group == root_id {
            // No parent graph exists for the root, so nothing could hold either a live edge or a
            // further mapping referencing it - this level is necessarily the end of the chain.
            levels.push(RemovedPortMapLevel {
                group_id: cur_group,
                parent_group_id: cur_group,
                external_port_name: cur_name,
                internal_node_id,
                internal_port_name,
                port_type,
            });
            break;
        }
        let (_, parent_id) = scenery.node_recursive(cur_group)?;
        levels.push(RemovedPortMapLevel {
            group_id: cur_group,
            parent_group_id: parent_id,
            external_port_name: cur_name.clone(),
            internal_node_id,
            internal_port_name,
            port_type,
        });

        let live_connections = scenery.with_group_node_mut(parent_id, |g| {
            let connections = g
                .graph()
                .get_connection_info_of_node(cur_group)
                .iter()
                .map(|c| ConnectInfo::from_connection_info(c, false))
                .filter(|c| match port_type {
                    PortType::Output => c.src_uuid() == cur_group && c.src_port() == cur_name,
                    PortType::Input => c.target_uuid() == cur_group && c.target_port() == cur_name,
                })
                .collect::<Vec<ConnectInfo>>();
            for c in &connections {
                g.disconnect_nodes(c.src_uuid(), c.src_port())?;
            }
            Ok::<Vec<ConnectInfo>, OpossumError>(connections)
        })??;

        if !live_connections.is_empty() {
            disconnected_connections.extend(live_connections.into_iter().map(|c| (parent_id, c)));
            break;
        }

        let outer_name = scenery.with_group_node(parent_id, |g| {
            g.graph()
                .port_map(&port_type)
                .external_port_of_mapped_port(cur_group, &cur_name)
        })?;

        match outer_name {
            Some(name) => {
                cur_group = parent_id;
                cur_name = name;
            }
            None => break,
        }
    }

    Ok(Some(PortMapCascadeRemoval {
        levels,
        disconnected_connections,
    }))
}

/// What ultimately consumes a pre-existing external mapping on the group a node is being moved out
/// of, as discovered by [`find_pre_existing_mapping_consumer`].
enum PreExistingMappingConsumer {
    /// A live connection consumes the export, found in `holder_group_id`'s own graph - which may be
    /// more than one hop out from where the walk started, if the mapping was already chained
    /// through one or more intermediate groups by an earlier move/conversion.
    LiveEdge {
        holder_group_id: Uuid,
        edge: ConnectionInfo,
    },
    /// The chain terminates by being re-exposed one more hop out, directly on the move's own
    /// `to_group_id` - once the moved node lands there, the two-level chain collapses to one.
    Collapse { outer_name: String },
    /// Nothing consumes this export at all - it'll be silently pruned once the moved node leaves
    /// its current group (`PortMap::remove_all_from_uuid`).
    Orphaned,
}

/// Walks outward from `from_group_id`'s own port-map entry (`external_name`, `port_type`) to find
/// whatever ultimately consumes it, for [`disconnect_moved_node_connections`]. Read-only - unlike
/// [`remove_port_map_cascade`], this has no "current level's own entry" to remove first, since
/// `from_group_id`'s entry is exactly what the caller is about to act on based on the result.
///
/// Per level: a live connection at the current group's immediate parent wins outright
/// ([`PreExistingMappingConsumer::LiveEdge`]). Otherwise, if the current group's export is itself
/// re-exposed one hop further out, and that parent is `to_group_id`, the chain collapses
/// ([`PreExistingMappingConsumer::Collapse`]); if it's re-exposed anywhere else, the walk continues
/// from there; if it isn't re-exposed at all, the export is unused ([`PreExistingMappingConsumer::Orphaned`]).
/// The scenery root is treated as the end of the line the same way, since it has no parent graph to
/// hold either a live edge or a further mapping.
///
/// Unlike `disconnect_stale_external_connections_for_node` (used by plain node deletion, a
/// different code path), which only ever looks one hop up, this walk continues arbitrarily far
/// outward - needed because a repeated convert-to-group (convert into a new group, then convert
/// again from inside it) can leave a pre-existing mapping chained through 2+ levels before this
/// ever runs, not just the single level a single drag-and-drop move produces.
///
/// Always terminates and never revisits a group, for the same reason [`remove_port_map_cascade`]
/// does: containment is a strict tree, so each step (moving to the current group's own parent)
/// strictly decreases distance-to-root.
///
/// # Errors
///
/// Returns an error if `from_group_id` or any outer group along the walk doesn't resolve.
fn find_pre_existing_mapping_consumer(
    scenery: &NodeGroup,
    from_group_id: Uuid,
    to_group_id: Uuid,
    external_name: &str,
    port_type: PortType,
) -> OpmResult<PreExistingMappingConsumer> {
    let mut cur_group = from_group_id;
    let mut cur_name = external_name.to_string();

    loop {
        let root_id = scenery.node_attr().uuid();
        if cur_group == root_id {
            return Ok(PreExistingMappingConsumer::Orphaned);
        }
        let (_, parent_id) = scenery.node_recursive(cur_group)?;

        let consumer = scenery.with_group_node(parent_id, |g| {
            g.graph()
                .get_connection_info_of_node(cur_group)
                .into_iter()
                .find(|c| match port_type {
                    PortType::Output => c.src_id == cur_group && c.src_port == cur_name,
                    PortType::Input => c.target_id == cur_group && c.target_port == cur_name,
                })
        })?;
        if let Some(edge) = consumer {
            return Ok(PreExistingMappingConsumer::LiveEdge {
                holder_group_id: parent_id,
                edge,
            });
        }

        let outer_name = scenery.with_group_node(parent_id, |g| {
            g.graph()
                .port_map(&port_type)
                .external_port_of_mapped_port(cur_group, &cur_name)
        })?;

        match outer_name {
            Some(name) if parent_id == to_group_id => {
                return Ok(PreExistingMappingConsumer::Collapse { outer_name: name });
            }
            Some(name) => {
                cur_group = parent_id;
                cur_name = name;
            }
            None => return Ok(PreExistingMappingConsumer::Orphaned),
        }
    }
}

/// What changed to keep a connection alive across a node's move to a different group, in place of
/// disconnecting it.
pub struct PreservedConnections {
    /// Connections newly created as a side effect - a boundary sibling reconnected to the destination's
    /// new mapped port, or a direct reconnect when the connection's other endpoint already lived in the
    /// destination - paired with the group each lives in.
    pub new_connections: Vec<(Uuid, ConnectInfo)>,
    /// Groups whose port-map/exposed-port display changed and need a GUI refresh.
    pub port_map_groups_changed: Vec<Uuid>,
    /// `(group_id, internal_node_id, external_port_name, port_type)` per port-map entry removed with no
    /// replacement under the same external name (the "collapse" case - reconnecting directly made the
    /// mapping unnecessary) - lets the GUI prune exactly this entry from its own cached port-map list,
    /// since a purely additive refresh wouldn't otherwise notice a key that's simply gone.
    pub removed_port_mappings: Vec<(Uuid, Uuid, String, PortType)>,
}

/// One connection or mapping that was torn down by [`disconnect_moved_node_connections`] and still needs
/// to be re-established by [`reconnect_moved_node_connections`] once the moved node actually exists in
/// `to_group_id`.
pub enum PendingReconnect {
    /// A connection to re-establish, either directly (`other_node_id` already lives in `to_group_id`) or
    /// via a fresh mapping on `to_group_id` - covers both a boundary sibling edge (case a) and a
    /// pre-existing mapping on `from_group_id` whose consumer is a live edge one hop up (case b).
    Edge {
        moved_node_id: Uuid,
        /// The moved node's own internal port name.
        moved_port: String,
        /// `Input` means the moved node's port was the connection's target (something fed into it);
        /// `Output` means it was the source - determines both the reconnect direction and which of
        /// `map_input_port`/`map_output_port` to use.
        port_type: PortType,
        other_node_id: Uuid,
        other_port: String,
        distance: f64,
        /// Where `other_node_id` lived at disconnect time - stable across the move, since `other_node_id`
        /// is never itself one of the moved nodes. `other_parent_id == to_group_id` is what an undo of a
        /// previous move looks like.
        other_parent_id: Uuid,
        /// Set only when this came from a pre-existing external mapping on `from_group_id` (case b) - the
        /// external name that exposed it, so the reconnect phase knows whether to drop it (collapse) or
        /// re-point it at the destination's freshly mapped port (reroute), rather than reconnecting a
        /// direct sibling edge (case a, `None`).
        from_group_external_name: Option<String>,
    },
    /// A pre-existing mapping on `from_group_id` (case b) whose "consumer" isn't a live edge but another
    /// mapping one level up, on `grandparent_id` (== `to_group_id`, guaranteed by the single-level move
    /// invariant): `from_group_id` is itself a subgroup nested inside `to_group_id`, so exposing its own
    /// member's port further outward necessarily chains through `from_group_id`'s own port map first. Once
    /// the moved node lands directly in `to_group_id`, that two-level chain collapses to one: the
    /// grandparent's own `outer_name` entry is re-pointed straight at the moved node's port.
    MappingCollapse {
        moved_node_id: Uuid,
        internal_port_name: String,
        port_type: PortType,
        grandparent_id: Uuid,
        outer_name: String,
    },
    /// A pre-existing mapping on `from_group_id` (case b) with no live consumer anywhere outward,
    /// preserved anyway because `to_group_id` is `from_group_id`'s own child: `from_group_id` isn't
    /// going anywhere, so its own export stays just as meaningful as before, regardless of whether
    /// anything happens to be plugged into it right now.
    MappingReroute {
        moved_node_id: Uuid,
        internal_port_name: String,
        port_type: PortType,
        external_name: String,
    },
}

/// Builds a [`ConnectInfo`] from raw connection endpoints, enriching it with whether the target
/// node is a reference node (see [`is_reference_target`]) - the flag the GUI uses to style
/// reference edges differently.
pub fn build_connect_info(
    scenery: &NodeGroup,
    src_id: Uuid,
    src_port: &str,
    target_id: Uuid,
    target_port: &str,
    distance: f64,
) -> ConnectInfo {
    let is_reference = is_reference_target(scenery, target_id);
    ConnectInfo::new(
        src_id,
        src_port.to_string(),
        target_id,
        target_port.to_string(),
        distance,
        is_reference,
    )
}

/// Returns `base` if `port_map` has no external port of that name yet, otherwise the first free
/// numbered variant (`base_2`, `base_3`, ...) - used when auto-creating a port mapping whose
/// preferred external name may already be taken on the target group.
fn generate_unique_external_name(port_map: &PortMap, base: &str) -> String {
    if !port_map.contains_external_name(base) {
        return base.to_string();
    }
    (2..10001)
        .map(|n| format!("{base}_{n}"))
        .find(|name| !port_map.contains_external_name(name))
        .expect("bounded search for free port name within 10000 attempts")
}

/// Reroutes `from_group_id`'s pre-existing mapping of `moved_node_id`'s `moved_port` (exposed under
/// `external_name`) to point at `to_group_id` instead, now that `moved_node_id` lives there - via a
/// freshly generated external name on `to_group_id` itself, so `from_group_id`'s own external name
/// stays completely unchanged. Used by [`reconnect_moved_node_connections`] both when the mapping's
/// consumer is a live edge one hop up and when there's no live consumer at all but `to_group_id` is
/// `from_group_id`'s own child, so the mapping is worth preserving anyway. The caller is responsible
/// for recording both groups in its own `port_map_groups_changed` list.
///
/// # Errors
///
/// Returns an error if `from_group_id`/`to_group_id` don't resolve or a `map_input_port`/
/// `map_output_port` step fails.
fn reroute_pre_existing_mapping(
    scenery: &mut NodeGroup,
    from_group_id: Uuid,
    to_group_id: Uuid,
    moved_node_id: Uuid,
    moved_port: &str,
    port_type: PortType,
    external_name: &str,
) -> OpmResult<()> {
    let to_group_port_map =
        scenery.with_group_node(to_group_id, |g| g.graph().port_map(&port_type).clone())?;
    let new_name = generate_unique_external_name(&to_group_port_map, moved_port);
    scenery.with_group_node_mut(to_group_id, |g| {
        map_port(g, port_type, moved_node_id, moved_port, &new_name)
    })??;
    scenery.with_group_node_mut(from_group_id, |g| {
        map_port(g, port_type, to_group_id, &new_name, external_name)
    })??;
    Ok(())
}

/// The pending reconnects captured by [`disconnect_moved_node_connections`], paired with the connections
/// it tore down outright (each `(group_id, ConnectInfo)`) so the caller can report both to the GUI.
pub type DisconnectedMovedNodeConnections = (Vec<PendingReconnect>, Vec<(Uuid, ConnectInfo)>);

/// Disconnects one boundary connection (a direct edge to a sibling staying behind in `from_group_id`)
/// and builds the [`PendingReconnect::Edge`] that re-establishes it once the moved node lands in
/// `to_group_id`. One iteration of [`disconnect_moved_node_connections`]'s boundary-connection loop.
///
/// # Errors
///
/// Returns an error if the connection's other endpoint doesn't resolve, or `disconnect_nodes` fails.
fn disconnect_boundary_connection(
    scenery: &mut NodeGroup,
    from_group_id: Uuid,
    c: &ConnectInfo,
    moved_node_ids: &[Uuid],
) -> OpmResult<(PendingReconnect, (Uuid, ConnectInfo))> {
    let moved_is_target = moved_node_ids.contains(&c.target_uuid());
    let (moved_node_id, moved_port, other_node_id, other_port) = if moved_is_target {
        (
            c.target_uuid(),
            c.target_port().to_string(),
            c.src_uuid(),
            c.src_port().to_string(),
        )
    } else {
        (
            c.src_uuid(),
            c.src_port().to_string(),
            c.target_uuid(),
            c.target_port().to_string(),
        )
    };
    let port_type = if moved_is_target {
        PortType::Input
    } else {
        PortType::Output
    };
    let (_, other_parent_id) = scenery.node_recursive(other_node_id)?;

    scenery.with_group_node_mut(from_group_id, |g| {
        g.disconnect_nodes(c.src_uuid(), c.src_port())
    })??;

    Ok((
        PendingReconnect::Edge {
            moved_node_id,
            moved_port,
            port_type,
            other_node_id,
            other_port,
            distance: c.distance(),
            other_parent_id,
            from_group_external_name: None,
        },
        (from_group_id, c.clone()),
    ))
}

/// One pre-existing external mapping site for [`disconnect_pre_existing_mapping`] to process -
/// bundled into a struct since `disconnect_moved_node_connections` otherwise has to thread through
/// more loose parameters than fit comfortably in a positional argument list.
struct PreExistingMappingSite {
    from_group_id: Uuid,
    to_group_id: Uuid,
    to_group_is_child_of_from_group: bool,
    moved_node_id: Uuid,
    port_type: PortType,
    external_name: String,
    internal_port_name: String,
}

/// The pending reconnect and/or torn-down connection produced by disconnecting one pre-existing
/// mapping or boundary connection - `None` in either slot where that particular outcome doesn't apply.
type MappingDisconnectOutcome = (Option<PendingReconnect>, Option<(Uuid, ConnectInfo)>);

/// Handles one pre-existing external mapping of `site.moved_node_id`'s `site.port_type` port on
/// `site.from_group_id` (found by walking [`find_pre_existing_mapping_consumer`]), returning whatever
/// [`disconnect_moved_node_connections`] needs to push into its own `pending`/`removed_connections`
/// accumulators - one iteration of its pre-existing-mapping loop.
///
/// # Errors
///
/// Returns an error if `from_group_id`/`to_group_id` don't resolve, the mapping's consumer can't be
/// found, or a `remove_mapped_port`/`disconnect_nodes`/`node_recursive` step fails.
fn disconnect_pre_existing_mapping(
    scenery: &mut NodeGroup,
    site: PreExistingMappingSite,
) -> OpmResult<MappingDisconnectOutcome> {
    let PreExistingMappingSite {
        from_group_id,
        to_group_id,
        to_group_is_child_of_from_group,
        moved_node_id,
        port_type,
        external_name,
        internal_port_name,
    } = site;
    let consumer = find_pre_existing_mapping_consumer(
        scenery,
        from_group_id,
        to_group_id,
        &external_name,
        port_type,
    )?;
    match consumer {
        PreExistingMappingConsumer::Orphaned => {
            if to_group_is_child_of_from_group {
                // `from_group_id` isn't losing this member for good - it's only moving one level
                // deeper inside `from_group_id`'s own subtree - so the export is preserved by
                // rerouting it, exactly as the `LiveEdge` case does, just without an outer edge to
                // also account for.
                scenery.with_group_node_mut(from_group_id, |g| {
                    g.remove_mapped_port(&external_name, port_type)
                })?;
                Ok((
                    Some(PendingReconnect::MappingReroute {
                        moved_node_id,
                        internal_port_name,
                        port_type,
                        external_name,
                    }),
                    None,
                ))
            } else {
                // The member is genuinely leaving `from_group_id` for good, and nothing anywhere
                // consumes this export - it'll be silently pruned once it does
                // (`PortMap::remove_all_from_uuid`).
                Ok((None, None))
            }
        }
        PreExistingMappingConsumer::Collapse { outer_name } => {
            // Only ever reachable on the walk's first hop: both callers guarantee `to_group_id` is
            // exactly one level adjacent to `from_group_id`, so `to_group_id` can't also be 2+ levels
            // out. That means this is always `from_group_id`'s own entry collapsing directly into
            // `to_group_id`'s.
            scenery.with_group_node_mut(from_group_id, |g| {
                g.remove_mapped_port(&external_name, port_type)
            })?;
            scenery.with_group_node_mut(to_group_id, |g| {
                g.remove_mapped_port(&outer_name, port_type)
            })?;
            Ok((
                Some(PendingReconnect::MappingCollapse {
                    moved_node_id,
                    internal_port_name,
                    port_type,
                    grandparent_id: to_group_id,
                    outer_name,
                }),
                None,
            ))
        }
        PreExistingMappingConsumer::LiveEdge {
            holder_group_id,
            edge,
        } => {
            let (other_node_id, other_port) = match port_type {
                PortType::Output => (edge.target_id, edge.target_port.clone()),
                PortType::Input => (edge.src_id, edge.src_port.clone()),
            };
            let (_, other_parent_id) = scenery.node_recursive(other_node_id)?;

            // The old entry referenced `moved_node_id` directly, which is about to move away -
            // remove it now so the reconnect phase can re-add the same external name (in either
            // branch) without colliding with it (`map_input_port`/`map_output_port` refuse to
            // overwrite an existing name).
            scenery.with_group_node_mut(from_group_id, |g| {
                g.remove_mapped_port(&external_name, port_type)
            })?;

            // Only the "collapse" case (the other endpoint already lives in `to_group_id`) actually
            // needs the outer edge itself torn down - it becomes a direct sibling connection instead.
            // In the common "reroute" case the external name stays exactly as it was; only what it
            // resolves to internally changes (in the reconnect phase), so the edge referencing that
            // name is left completely untouched here.
            let removed_connection = if other_parent_id == to_group_id {
                scenery.with_group_node_mut(holder_group_id, |g| {
                    g.disconnect_nodes(edge.src_id, &edge.src_port)
                })??;
                Some((
                    holder_group_id,
                    ConnectInfo::from_connection_info(&edge, false),
                ))
            } else {
                None
            };

            Ok((
                Some(PendingReconnect::Edge {
                    moved_node_id,
                    moved_port: internal_port_name,
                    port_type,
                    other_node_id,
                    other_port,
                    distance: edge.distance.value,
                    other_parent_id,
                    from_group_external_name: Some(external_name),
                }),
                removed_connection,
            ))
        }
    }
}

/// Before `moved_node_ids` (currently living in `from_group_id`) are deleted from it, tears down every
/// connection/mapping that would otherwise dangle once they're gone - a direct connection to a sibling
/// left behind in `from_group_id` (`boundary_connections` - the `input`/`output` halves of a
/// `ConnectionSplit`, computed by the caller via `split_sort_connections`/`split_sort_connections_from_document`),
/// or a pre-existing external mapping of one of the moved nodes' own ports on `from_group_id`, chained
/// outward through as many re-exporting groups as necessary until it reaches whatever ultimately consumes
/// it - a live connection, or nothing at all (see [`find_pre_existing_mapping_consumer`], which performs
/// this walk). Unlike `disconnect_stale_external_connections_for_node` (used by plain node deletion, a
/// different code path, which only ever looks one hop up and just disconnects and forgets), this returns a
/// [`PendingReconnect`] per torn-down link, to be handed to [`reconnect_moved_node_connections`] *after*
/// the caller has actually moved the nodes into `to_group_id` - `map_input_port`/`map_output_port` and a
/// direct reconnect inside `to_group_id` both require the moved node to already be present there, which
/// isn't true yet at this point.
///
/// `to_group_id` is used only to tell a "collapse" (the other endpoint already lives there - what undoing
/// a previous move looks like, so the external link becomes unnecessary and is torn down for good) from a
/// "reroute" (the external link's own name/edge is left completely untouched, only its internal target
/// changes later) - nothing is written to `to_group_id` itself at this point.
///
/// # Errors
///
/// Returns an error if `from_group_id` doesn't resolve, a node/port can't be resolved, or a
/// `disconnect_nodes`/`remove_mapped_port` step fails.
pub fn disconnect_moved_node_connections(
    scenery: &mut NodeGroup,
    from_group_id: Uuid,
    to_group_id: Uuid,
    boundary_connections: &[ConnectInfo],
    moved_node_ids: &[Uuid],
) -> OpmResult<DisconnectedMovedNodeConnections> {
    let mut pending = Vec::new();
    let mut removed_connections = Vec::new();
    // Whether `from_group_id` itself persists as the moved nodes' new ancestor (true for
    // convert-to-group, which always creates `to_group_id` as a fresh child of `from_group_id`; may
    // also hold for a drag-and-drop move into an existing child subgroup). When it does,
    // `from_group_id`'s own pre-existing export of a moved node stays just as meaningful as before
    // even if nothing outward currently consumes it - see the `Orphaned` arm below.
    let to_group_is_child_of_from_group = scenery
        .node_recursive(to_group_id)
        .is_ok_and(|(_, parent)| parent == from_group_id);

    // A direct connection to a sibling staying behind in `from_group_id`.
    for c in boundary_connections {
        let (reconnect, removed) =
            disconnect_boundary_connection(scenery, from_group_id, c, moved_node_ids)?;
        pending.push(reconnect);
        removed_connections.push(removed);
    }

    // A pre-existing external mapping of one of the moved nodes' own ports on `from_group_id`.
    for moved_node_id in moved_node_ids {
        for port_type in [PortType::Input, PortType::Output] {
            let assigned = scenery.with_group_node(from_group_id, |g| {
                g.graph()
                    .port_map(&port_type)
                    .assigned_ports_for_node(*moved_node_id)
            })?;
            for (external_name, internal_port_name) in assigned {
                let (reconnect, removed) = disconnect_pre_existing_mapping(
                    scenery,
                    PreExistingMappingSite {
                        from_group_id,
                        to_group_id,
                        to_group_is_child_of_from_group,
                        moved_node_id: *moved_node_id,
                        port_type,
                        external_name,
                        internal_port_name,
                    },
                )?;
                if let Some(reconnect) = reconnect {
                    pending.push(reconnect);
                }
                if let Some(removed) = removed {
                    removed_connections.push(removed);
                }
            }
        }
    }

    Ok((pending, removed_connections))
}

/// The moved-node/other-endpoint data carried by a [`PendingReconnect::Edge`], destructured out so
/// [`reconnect_edge`] can take it as a single unit.
struct EdgeReconnect {
    moved_node_id: Uuid,
    moved_port: String,
    port_type: PortType,
    other_node_id: Uuid,
    other_port: String,
    distance: f64,
    other_parent_id: Uuid,
    from_group_external_name: Option<String>,
}

/// What reconnecting one [`PendingReconnect::Edge`] contributes to
/// [`reconnect_moved_node_connections`]'s overall [`PreservedConnections`] result.
struct EdgeReconnectOutcome {
    new_connection: Option<(Uuid, ConnectInfo)>,
    port_map_groups_changed: Vec<Uuid>,
    removed_port_mapping: Option<(Uuid, Uuid, String, PortType)>,
}

/// Re-establishes one torn-down edge: either a direct reconnect inside `to_group_id` (the connection's
/// other endpoint already lives there - what undoing a previous move looks like), a reroute of a
/// pre-existing mapping onto the destination, or a freshly created port mapping on `to_group_id` with
/// the link rerouted through it. One iteration of [`reconnect_moved_node_connections`]'s handling of
/// the `PendingReconnect::Edge` variant.
///
/// # Errors
///
/// Returns an error if `from_group_id`/`to_group_id` don't resolve, the moved node's port can't be
/// resolved, or a `connect_nodes`/`map_input_port`/`map_output_port` step fails.
fn reconnect_edge(
    scenery: &mut NodeGroup,
    from_group_id: Uuid,
    to_group_id: Uuid,
    edge: EdgeReconnect,
) -> OpmResult<EdgeReconnectOutcome> {
    let EdgeReconnect {
        moved_node_id,
        moved_port,
        port_type,
        other_node_id,
        other_port,
        distance,
        other_parent_id,
        from_group_external_name,
    } = edge;

    if other_parent_id == to_group_id {
        let (src_id, src_port, target_id, target_port) = match port_type {
            PortType::Input => (other_node_id, other_port, moved_node_id, moved_port),
            PortType::Output => (moved_node_id, moved_port, other_node_id, other_port),
        };
        scenery.with_group_node_mut(to_group_id, |g| {
            g.connect_nodes(src_id, &src_port, target_id, &target_port, meter!(distance))
        })??;
        let new_info = build_connect_info(
            scenery,
            src_id,
            &src_port,
            target_id,
            &target_port,
            distance,
        );
        let (port_map_groups_changed, removed_port_mapping) =
            from_group_external_name.as_ref().map_or_else(
                || (vec![], None),
                |external_name| {
                    (
                        vec![from_group_id],
                        Some((
                            from_group_id,
                            moved_node_id,
                            external_name.clone(),
                            port_type,
                        )),
                    )
                },
            );
        Ok(EdgeReconnectOutcome {
            new_connection: Some((to_group_id, new_info)),
            port_map_groups_changed,
            removed_port_mapping,
        })
    } else if let Some(external_name) = from_group_external_name {
        reroute_pre_existing_mapping(
            scenery,
            from_group_id,
            to_group_id,
            moved_node_id,
            &moved_port,
            port_type,
            &external_name,
        )?;
        Ok(EdgeReconnectOutcome {
            new_connection: None,
            port_map_groups_changed: vec![to_group_id, from_group_id],
            removed_port_mapping: None,
        })
    } else {
        let to_group_port_map =
            scenery.with_group_node(to_group_id, |g| g.graph().port_map(&port_type).clone())?;
        let new_name = generate_unique_external_name(&to_group_port_map, &moved_port);
        scenery.with_group_node_mut(to_group_id, |g| {
            map_port(g, port_type, moved_node_id, &moved_port, &new_name)
        })??;

        let (src_id, src_port, target_id, target_port) = match port_type {
            PortType::Input => (other_node_id, other_port, to_group_id, new_name),
            PortType::Output => (to_group_id, new_name, other_node_id, other_port),
        };
        scenery.with_group_node_mut(from_group_id, |g| {
            g.connect_nodes(src_id, &src_port, target_id, &target_port, meter!(distance))
        })??;
        let new_info = build_connect_info(
            scenery,
            src_id,
            &src_port,
            target_id,
            &target_port,
            distance,
        );
        Ok(EdgeReconnectOutcome {
            new_connection: Some((from_group_id, new_info)),
            port_map_groups_changed: vec![to_group_id],
            removed_port_mapping: None,
        })
    }
}

/// After `pending`'s moved nodes have actually been re-added to `to_group_id`, re-establishes each torn-
/// down link captured by [`disconnect_moved_node_connections`] - either a direct reconnect inside
/// `to_group_id` (when the connection's other endpoint already lives there - what undoing a previous move
/// looks like) or a freshly created port mapping on `to_group_id` with the link rerouted through it.
///
/// Both callers (drag-and-drop moves and convert-to-group) guarantee `to_group_id` and
/// `from_group_id` are always direct parent/child of each other (in whichever direction), no matter
/// how many further levels a pre-existing mapping on `from_group_id` was itself already chained
/// through before this move - see [`find_pre_existing_mapping_consumer`], which does the
/// arbitrary-depth discovery on the disconnect side. This function only ever reads/writes
/// `from_group_id`'s and `to_group_id`'s own port maps, exactly as if the pending reconnect had been
/// discovered one hop up, so it doesn't need to know how far out that discovery walk actually went -
/// a single "does the connection's other endpoint already live in `to_group_id`?" check is
/// sufficient to detect an undo-style move, with no need to track any state across calls.
///
/// # Errors
///
/// Returns an error if `from_group_id`/`to_group_id` don't resolve, a moved node's port can't be resolved,
/// or a `connect_nodes`/`map_input_port`/`map_output_port` step fails.
pub fn reconnect_moved_node_connections(
    scenery: &mut NodeGroup,
    from_group_id: Uuid,
    to_group_id: Uuid,
    pending: Vec<PendingReconnect>,
) -> OpmResult<PreservedConnections> {
    let mut result = PreservedConnections {
        new_connections: Vec::new(),
        port_map_groups_changed: Vec::new(),
        removed_port_mappings: Vec::new(),
    };

    for p in pending {
        match p {
            PendingReconnect::MappingCollapse {
                moved_node_id,
                internal_port_name,
                port_type,
                grandparent_id,
                outer_name,
            } => {
                scenery.with_group_node_mut(grandparent_id, |g| {
                    map_port(
                        g,
                        port_type,
                        moved_node_id,
                        &internal_port_name,
                        &outer_name,
                    )
                })??;
                result.port_map_groups_changed.push(grandparent_id);
            }
            PendingReconnect::MappingReroute {
                moved_node_id,
                internal_port_name,
                port_type,
                external_name,
            } => {
                reroute_pre_existing_mapping(
                    scenery,
                    from_group_id,
                    to_group_id,
                    moved_node_id,
                    &internal_port_name,
                    port_type,
                    &external_name,
                )?;
                result.port_map_groups_changed.push(to_group_id);
                result.port_map_groups_changed.push(from_group_id);
            }
            PendingReconnect::Edge {
                moved_node_id,
                moved_port,
                port_type,
                other_node_id,
                other_port,
                distance,
                other_parent_id,
                from_group_external_name,
            } => {
                let outcome = reconnect_edge(
                    scenery,
                    from_group_id,
                    to_group_id,
                    EdgeReconnect {
                        moved_node_id,
                        moved_port,
                        port_type,
                        other_node_id,
                        other_port,
                        distance,
                        other_parent_id,
                        from_group_external_name,
                    },
                )?;
                if let Some(new_connection) = outcome.new_connection {
                    result.new_connections.push(new_connection);
                }
                result
                    .port_map_groups_changed
                    .extend(outcome.port_map_groups_changed);
                if let Some(removed) = outcome.removed_port_mapping {
                    result.removed_port_mappings.push(removed);
                }
            }
        }
    }

    result.port_map_groups_changed.sort();
    result.port_map_groups_changed.dedup();
    Ok(result)
}

/// Split and classify connections relative to a set of nodes.
///
/// Connections are categorized into three groups:
/// - `inside`: connections where both source and target nodes are inside the set
/// - `input`: connections entering the set (target inside, source outside)
/// - `output`: connections leaving the set (source inside, target outside)
///
/// Additionally, each connection is annotated with whether its target node
/// represents a reference node.
///
/// # Arguments
///
/// * `connections` - Slice of all connections to evaluate.
/// * `nodes` - Slice of node UUIDs defining the subset of interest.
///
/// # Returns
///
/// Returns a [`ConnectionSplit`] struct containing the categorized connections.
///
/// # Errors
///
/// Missing node attributes are treated as non-reference nodes.
#[allow(clippy::significant_drop_tightening)]
pub fn split_sort_connections(
    data: &web::Data<AppState>,
    connections: &[ConnectionInfo],
    nodes: &[Uuid],
) -> ConnectionSplit {
    let document = data.document.lock();
    split_sort_connections_from_document(&document, connections, nodes)
}

/// Same classification as [`split_sort_connections`], but taking an already-locked `&OpmDocument`
/// directly instead of `&web::Data<AppState>` - for callers (like undo/redo command application) that
/// only have a document reference, not the full app state, and shouldn't re-lock it.
pub fn split_sort_connections_from_document(
    document: &OpmDocument,
    connections: &[ConnectionInfo],
    nodes: &[Uuid],
) -> ConnectionSplit {
    let node_set: HashSet<Uuid> = nodes.iter().copied().collect();

    let mut split = ConnectionSplit {
        inside: Vec::new(),
        input: Vec::new(),
        output: Vec::new(),
    };

    let scenery = document.scenery();
    for c in connections {
        let is_reference = is_reference_target(scenery, c.target_id);
        let c_info = ConnectInfo::from_connection_info(c, is_reference);

        let src_inside = node_set.contains(&c_info.src_uuid());
        let tgt_inside = node_set.contains(&c_info.target_uuid());

        match (src_inside, tgt_inside) {
            (true, true) => split.inside.push(c_info),
            (true, false) => split.output.push(c_info),
            (false, true) => split.input.push(c_info),
            _ => {}
        }
    }

    split
}

/// Represents a categorized split of connections.
///
/// # Fields
///
/// * `inside` - Connections fully contained within the node set.
/// * `input` - Connections entering the node set.
/// * `output` - Connections leaving the node set.
pub struct ConnectionSplit {
    pub inside: Vec<ConnectInfo>,
    pub input: Vec<ConnectInfo>,
    pub output: Vec<ConnectInfo>,
}

/// Connect two nodes within a group based on `ConnectInfo`.
///
/// This is a convenience helper that forwards connection data to
/// `NodeGroup::connect_nodes`.
///
/// # Arguments
///
/// * `group` - The group in which the connection should be created.
/// * `conn` - The connection description.
///
/// # Errors
///
/// This function will return an error if the connection cannot be created.
pub fn connect_from_info(group: &mut NodeGroup, conn: &ConnectInfo) -> OpmResult<()> {
    group.connect_nodes(
        conn.src_uuid(),
        conn.src_port(),
        conn.target_uuid(),
        conn.target_port(),
        meter!(conn.distance()),
    )
}

/// Reconnects every [`ConnectInfo`] in `connections` inside `group_id`'s graph, via
/// [`connect_from_info`].
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group, or a connection can't be re-created
/// (e.g. a referenced node/port no longer exists).
pub fn reconnect_all(
    document: &mut OpmDocument,
    group_id: Uuid,
    connections: &[ConnectInfo],
) -> Result<(), BackEndErrorResponse> {
    for conn in connections {
        document
            .scenery_mut()
            .with_group_node_mut(group_id, |g| connect_from_info(g, conn))??;
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

/// Custom extractor to handle Rusty Object Notation (RON) payloads
pub struct Ron<T>(pub T);

impl<T> Ron<T> {
    /// Deconstruct to get the inner value
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> FromRequest for Ron<T>
where
    T: DeserializeOwned + 'static,
{
    // Use your custom error response type directly
    type Error = BackEndErrorResponse;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        // Reuse Actix's built-in String extractor to read the request body
        let string_fut = String::from_request(req, payload);

        Box::pin(async move {
            // 1. Extract the raw string payload and map potential Actix errors
            let body_str = string_fut.await.map_err(|err| {
                BackEndErrorResponse::new(
                    400,
                    "Payload Error",
                    &format!("Failed to read request body: {err}"),
                )
            })?;

            // 2. Deserialize the RON string into the target type T
            let data = ron::de::from_str(&body_str).map_err(|err| {
                BackEndErrorResponse::new(
                    400,
                    "Parse Error",
                    &format!("Failed to deserialize payload: {err}"),
                )
            })?;

            Ok(Self(data))
        })
    }
}

/// Serializes `value` as the response body, honoring content negotiation between RON and JSON.
///
/// If `req`'s `Accept` header contains `application/ron`, `value` is serialized to RON (using
/// pretty formatting), since RON can represent `NaN`/`Inf` values that JSON cannot. Otherwise the
/// response falls back to JSON.
///
/// # Errors
///
/// Returns an error if RON serialization fails.
pub fn ron_or_json_response<T: Serialize>(
    req: &HttpRequest,
    value: &T,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let wants_ron = req
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|s| s.contains("application/ron"));

    if wants_ron {
        let body = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::new().new_line("\n"))
            .map_err(|e| BackEndErrorResponse::new(500, "Serialization Error", &e.to_string()))?;
        Ok(HttpResponse::Ok()
            .content_type("application/ron")
            .body(body))
    } else {
        Ok(HttpResponse::Ok().json(value))
    }
}
