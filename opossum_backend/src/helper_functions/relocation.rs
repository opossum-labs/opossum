use opossum_core::{
    core_optics::OpticRef, error::OpmResult, nodes::NodeGroup, opm_document::OpmDocument,
    types::api_types::ConnectInfo,
};
use uuid::Uuid;

use super::{
    connection_classification::{connect_from_info, split_sort_connections_from_document},
    connection_preservation::{
        PreservedConnections, disconnect_moved_node_connections, reconnect_moved_node_connections,
    },
    graph_lookup::validate_relocated_references,
    port_map_cascade::{PortMapCascadeRemoval, disconnect_exposed_port_cascades_for_node},
};

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
/// Returns an error if either group id doesn't resolve, a moved node's uuid can't be found, or the move
/// would place a `NodeReference` inside its own target group (or a group nested within it) - see
/// [`validate_relocated_references`].
pub fn relocate_nodes_in_document(
    document: &mut OpmDocument,
    from_group_id: Uuid,
    to_group_id: Uuid,
    node_ids: &[Uuid],
) -> OpmResult<RelocationOutcome> {
    validate_relocated_references(document.scenery(), node_ids, to_group_id)?;

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
/// Returns an error if either group id doesn't resolve, a moved node's uuid can't be found, a
/// severing / connection step fails, or the relocation would place a `NodeReference` inside its own
/// target group (or a group nested within it) - see
/// [`validate_relocated_references`].
pub fn relocate_nodes_severing_external_links(
    document: &mut OpmDocument,
    from_group_id: Uuid,
    to_group_id: Uuid,
    node_ids: &[Uuid],
) -> OpmResult<CutRelocationOutcome> {
    validate_relocated_references(document.scenery(), node_ids, to_group_id)?;

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
