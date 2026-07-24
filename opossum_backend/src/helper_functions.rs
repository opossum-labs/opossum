use std::{collections::HashSet, pin::Pin};

use actix_web::{
    FromRequest, HttpRequest,
    dev::Payload,
    web::{self},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    meter,
    nodes::{ConnectionInfo, NodeGroup},
    opm_document::OpmDocument,
    prelude::{PortMap, PortType},
    types::api_types::{ConnectInfo, NodeInfo},
    utils::LockExt,
};
use serde::de::DeserializeOwned;
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
            let is_reference = scenery
                .with_node_attr(c.target_id, |attr| {
                    attr.properties().get("reference id").is_ok()
                })
                .unwrap_or(false);
            ConnectInfo::from_connection_info(c, is_reference)
        })
        .collect())
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

/// Everything needed to restore one external connection that had to be torn down because it depended on a
/// port mapping of a node that's about to be deleted.
pub struct DisconnectedPortMapping {
    /// The group whose port map exposed this connection (== the deleted node's own parent).
    pub mapping_group_id: Uuid,
    /// `mapping_group_id`'s own parent, where the external connection actually lived.
    pub mapping_parent_group_id: Uuid,
    pub internal_node_id: Uuid,
    pub internal_port_name: String,
    pub external_port_name: String,
    pub port_type: PortType,
    pub connect_info: ConnectInfo,
}

/// Splits a set of torn-down port mappings into the two response shapes callers report to the GUI: the
/// external connections that were disconnected, and the port-map entries that were removed.
///
/// Used by both `delete_node` and `post_paste_nodes`'s cut branch, since both tear down mappings via
/// [`disconnect_stale_external_connections_for_node`] and report the result the same way.
pub fn split_disconnected_mappings_for_response(
    mappings: &[DisconnectedPortMapping],
) -> (
    Vec<(Uuid, ConnectInfo)>,
    Vec<(Uuid, Uuid, String, PortType)>,
) {
    let disconnected_connections = mappings
        .iter()
        .map(|d| (d.mapping_parent_group_id, d.connect_info.clone()))
        .collect();
    // Also carries the external port name + type, so the GUI can shrink the group's own
    // exposed-port handles precisely (`remove_group_port`) instead of re-fetching them.
    let removed_port_mappings = mappings
        .iter()
        .map(|d| {
            (
                d.mapping_group_id,
                d.internal_node_id,
                d.external_port_name.clone(),
                d.port_type,
            )
        })
        .collect();
    (disconnected_connections, removed_port_mappings)
}

/// Before `node_id` (living in `parent_group_id`) is deleted, disconnects any external connection in
/// `parent_group_id`'s own parent graph that depends on a port mapping of one of `node_id`'s ports - i.e.
/// `parent_group_id` exposes one of `node_id`'s ports externally under some name, and a sibling node
/// outside `parent_group_id` is wired to that external port. Mirrors `apply_remove_port_map`'s
/// find-then-disconnect steps (`undo/port_map_commands.rs`), but triggered by "this node is about to be
/// deleted" rather than "this specific mapping is being removed by name," and runs for every mapping the
/// node has, in both directions.
///
/// Deliberately does not remove the port-map entries themselves - the caller's subsequent
/// `NodeGroup::delete_node` already prunes them via `PortMap::remove_all_from_uuid`. This is only about
/// tearing down the now-stale external edge before that happens, so it doesn't outlive the mapping it
/// depends on.
///
/// `scenery` must be the document root (`document.scenery_mut()`) - `parent_group_id`'s own parent is
/// resolved via `scenery.node_recursive`, and if `parent_group_id` *is* the root there is no grandparent to
/// look in, so this returns `Ok(vec![])` immediately without a lookup.
///
/// # Errors
///
/// Returns an error if `parent_group_id` doesn't resolve to a group, or if disconnecting a found
/// connection fails.
pub fn disconnect_stale_external_connections_for_node(
    scenery: &mut NodeGroup,
    parent_group_id: Uuid,
    node_id: Uuid,
) -> OpmResult<Vec<DisconnectedPortMapping>> {
    if parent_group_id == scenery.node_attr().uuid() {
        return Ok(Vec::new());
    }

    let mapped: Vec<(PortType, String, String)> =
        scenery.with_group_node(parent_group_id, |g| {
            [PortType::Input, PortType::Output]
                .into_iter()
                .flat_map(|port_type| {
                    g.graph()
                        .port_map(&port_type)
                        .assigned_ports_for_node(node_id)
                        .into_iter()
                        .map(move |(external_name, internal_name)| {
                            (port_type, external_name, internal_name)
                        })
                })
                .collect::<Vec<_>>()
        })?;

    if mapped.is_empty() {
        return Ok(Vec::new());
    }

    let (_, grandparent_id) = scenery.node_recursive(parent_group_id)?;

    let mut disconnected = Vec::new();
    for (port_type, external_port_name, internal_port_name) in mapped {
        let connections = scenery.with_group_node_mut(grandparent_id, |g| {
            let connections = g
                .graph()
                .get_connection_info_of_node(parent_group_id)
                .iter()
                .map(|c| ConnectInfo::from_connection_info(c, false))
                .filter(|c| match port_type {
                    PortType::Output => {
                        c.src_uuid() == parent_group_id && c.src_port() == external_port_name
                    }
                    PortType::Input => {
                        c.target_uuid() == parent_group_id && c.target_port() == external_port_name
                    }
                })
                .collect::<Vec<ConnectInfo>>();
            for c in &connections {
                g.disconnect_nodes(c.src_uuid(), c.src_port())?;
            }
            Ok::<Vec<ConnectInfo>, OpossumError>(connections)
        })??;

        for connect_info in connections {
            disconnected.push(DisconnectedPortMapping {
                mapping_group_id: parent_group_id,
                mapping_parent_group_id: grandparent_id,
                internal_node_id: node_id,
                internal_port_name: internal_port_name.clone(),
                external_port_name: external_port_name.clone(),
                port_type,
                connect_info,
            });
        }
    }

    Ok(disconnected)
}

/// One level of a cascading port-map removal - one group's own mapping entry that was removed,
/// with enough captured to recreate it on undo.
pub struct RemovedPortMapLevel {
    pub group_id: Uuid,
    pub parent_group_id: Uuid,
    pub external_port_name: String,
    pub internal_node_id: Uuid,
    pub internal_port_name: String,
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
    /// `(group_id, node_id)` pairs whose port-map entry was removed with no replacement under the same
    /// external name (the "collapse" case - reconnecting directly made the mapping unnecessary) - lets
    /// the GUI prune exactly this entry from its own cached port-map list, since a purely additive
    /// refresh wouldn't otherwise notice a key that's simply gone.
    pub removed_port_mappings: Vec<(Uuid, Uuid)>,
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

fn build_connect_info(
    scenery: &NodeGroup,
    src_id: Uuid,
    src_port: &str,
    target_id: Uuid,
    target_port: &str,
    distance: f64,
) -> ConnectInfo {
    let is_reference = scenery
        .with_node_attr(target_id, |attr| {
            attr.properties().get("reference id").is_ok()
        })
        .unwrap_or(false);
    ConnectInfo::new(
        src_id,
        src_port.to_string(),
        target_id,
        target_port.to_string(),
        distance,
        is_reference,
    )
}

fn generate_unique_external_name(port_map: &PortMap, base: &str) -> String {
    if !port_map.contains_external_name(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}_{n}"))
        .find(|name| !port_map.contains_external_name(name))
        .expect("an unbounded search for a free name always terminates")
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
    match port_type {
        PortType::Input => scenery.with_group_node_mut(to_group_id, |g| {
            g.map_input_port(moved_node_id, moved_port, &new_name)
        })??,
        PortType::Output => scenery.with_group_node_mut(to_group_id, |g| {
            g.map_output_port(moved_node_id, moved_port, &new_name)
        })??,
    }

    match port_type {
        PortType::Input => scenery.with_group_node_mut(from_group_id, |g| {
            g.map_input_port(to_group_id, &new_name, external_name)
        })??,
        PortType::Output => scenery.with_group_node_mut(from_group_id, |g| {
            g.map_output_port(to_group_id, &new_name, external_name)
        })??,
    }
    Ok(())
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
/// The pending reconnects captured by [`disconnect_moved_node_connections`], paired with the connections
/// it tore down outright (each `(group_id, ConnectInfo)`) so the caller can report both to the GUI.
pub type DisconnectedMovedNodeConnections = (Vec<PendingReconnect>, Vec<(Uuid, ConnectInfo)>);

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
        removed_connections.push((from_group_id, c.clone()));

        pending.push(PendingReconnect::Edge {
            moved_node_id,
            moved_port,
            port_type,
            other_node_id,
            other_port,
            distance: c.distance(),
            other_parent_id,
            from_group_external_name: None,
        });
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
                            // `from_group_id` isn't losing this member for good - it's only moving
                            // one level deeper inside `from_group_id`'s own subtree - so the export
                            // is preserved by rerouting it, exactly as the `LiveEdge` case does,
                            // just without an outer edge to also account for.
                            scenery.with_group_node_mut(from_group_id, |g| {
                                g.remove_mapped_port(&external_name, port_type)
                            })?;
                            pending.push(PendingReconnect::MappingReroute {
                                moved_node_id: *moved_node_id,
                                internal_port_name,
                                port_type,
                                external_name,
                            });
                        }
                        // Otherwise the member is genuinely leaving `from_group_id` for good, and
                        // nothing anywhere consumes this export - it'll be silently pruned once it
                        // does (`PortMap::remove_all_from_uuid`).
                    }
                    PreExistingMappingConsumer::Collapse { outer_name } => {
                        // Only ever reachable on the walk's first hop: both callers guarantee
                        // `to_group_id` is exactly one level adjacent to `from_group_id`, so
                        // `to_group_id` can't also be 2+ levels out. That means this is always
                        // `from_group_id`'s own entry collapsing directly into `to_group_id`'s.
                        scenery.with_group_node_mut(from_group_id, |g| {
                            g.remove_mapped_port(&external_name, port_type)
                        })?;
                        scenery.with_group_node_mut(to_group_id, |g| {
                            g.remove_mapped_port(&outer_name, port_type)
                        })?;
                        pending.push(PendingReconnect::MappingCollapse {
                            moved_node_id: *moved_node_id,
                            internal_port_name,
                            port_type,
                            grandparent_id: to_group_id,
                            outer_name,
                        });
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

                        // The old entry referenced `moved_node_id` directly, which is about to move
                        // away - remove it now so the reconnect phase can re-add the same external
                        // name (in either branch) without colliding with it
                        // (`map_input_port`/`map_output_port` refuse to overwrite an existing name).
                        scenery.with_group_node_mut(from_group_id, |g| {
                            g.remove_mapped_port(&external_name, port_type)
                        })?;

                        // Only the "collapse" case (the other endpoint already lives in
                        // `to_group_id`) actually needs the outer edge itself torn down - it becomes
                        // a direct sibling connection instead. In the common "reroute" case the
                        // external name stays exactly as it was; only what it resolves to internally
                        // changes (in the reconnect phase), so the edge referencing that name is left
                        // completely untouched here.
                        if other_parent_id == to_group_id {
                            scenery.with_group_node_mut(holder_group_id, |g| {
                                g.disconnect_nodes(edge.src_id, &edge.src_port)
                            })??;
                            removed_connections.push((
                                holder_group_id,
                                ConnectInfo::from_connection_info(&edge, false),
                            ));
                        }

                        pending.push(PendingReconnect::Edge {
                            moved_node_id: *moved_node_id,
                            moved_port: internal_port_name,
                            port_type,
                            other_node_id,
                            other_port,
                            distance: edge.distance.value,
                            other_parent_id,
                            from_group_external_name: Some(external_name),
                        });
                    }
                }
            }
        }
    }

    Ok((pending, removed_connections))
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
        let (
            moved_node_id,
            moved_port,
            port_type,
            other_node_id,
            other_port,
            distance,
            other_parent_id,
            from_group_external_name,
        ) = match p {
            PendingReconnect::MappingCollapse {
                moved_node_id,
                internal_port_name,
                port_type,
                grandparent_id,
                outer_name,
            } => {
                match port_type {
                    PortType::Input => scenery.with_group_node_mut(grandparent_id, |g| {
                        g.map_input_port(moved_node_id, &internal_port_name, &outer_name)
                    })??,
                    PortType::Output => scenery.with_group_node_mut(grandparent_id, |g| {
                        g.map_output_port(moved_node_id, &internal_port_name, &outer_name)
                    })??,
                }
                result.port_map_groups_changed.push(grandparent_id);
                continue;
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
                continue;
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
            } => (
                moved_node_id,
                moved_port,
                port_type,
                other_node_id,
                other_port,
                distance,
                other_parent_id,
                from_group_external_name,
            ),
        };

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
            result.new_connections.push((to_group_id, new_info));
            if from_group_external_name.is_some() {
                result
                    .removed_port_mappings
                    .push((from_group_id, moved_node_id));
                result.port_map_groups_changed.push(from_group_id);
            }
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
            result.port_map_groups_changed.push(to_group_id);
            result.port_map_groups_changed.push(from_group_id);
        } else {
            let to_group_port_map =
                scenery.with_group_node(to_group_id, |g| g.graph().port_map(&port_type).clone())?;
            let new_name = generate_unique_external_name(&to_group_port_map, &moved_port);
            match port_type {
                PortType::Input => scenery.with_group_node_mut(to_group_id, |g| {
                    g.map_input_port(moved_node_id, &moved_port, &new_name)
                })??,
                PortType::Output => scenery.with_group_node_mut(to_group_id, |g| {
                    g.map_output_port(moved_node_id, &moved_port, &new_name)
                })??,
            }
            result.port_map_groups_changed.push(to_group_id);

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
            result.new_connections.push((from_group_id, new_info));
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
        let is_reference = scenery
            .with_node_attr(c.target_id, |attr| {
                attr.properties().get("reference id").is_ok()
            })
            .unwrap_or(false);

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
