use opossum_core::{
    core_optics::node_attr::HasNodeAttr,
    error::OpmResult,
    meter,
    nodes::{ConnectionInfo, NodeGroup},
    prelude::{PortMap, PortType},
    types::api_types::ConnectInfo,
};
use uuid::Uuid;

use super::{connection_classification::build_connect_info, graph_lookup::map_port};

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
pub(super) enum PendingReconnect {
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
pub(super) type DisconnectedMovedNodeConnections =
    (Vec<PendingReconnect>, Vec<(Uuid, ConnectInfo)>);

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
pub(super) fn disconnect_moved_node_connections(
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
pub(super) fn reconnect_moved_node_connections(
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
