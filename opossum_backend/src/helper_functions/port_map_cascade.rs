use opossum_core::{
    core_optics::node_attr::HasNodeAttr,
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
    prelude::PortType,
    types::api_types::ConnectInfo,
};
use uuid::Uuid;

type CascadeRemovalResult = (
    Vec<(Uuid, ConnectInfo)>,
    Vec<(Uuid, Uuid, String, PortType)>,
);

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
