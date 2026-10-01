use opossum_core::{
    core_optics::node_attr::HasNodeAttr,
    error::{OpmResult, OpossumError},
    nodes::{GraphDelta, NodeGroup},
    prelude::PortType,
    types::api_types::ConnectInfo,
};
use uuid::Uuid;

type CascadeRemovalResult = (
    Vec<(Uuid, ConnectInfo)>,
    Vec<(Uuid, Uuid, String, PortType)>,
);

/// Flattens the cascades torn down by `remove_port_map_cascade` into the two
/// response shapes the GUI consumes: every external connection disconnected and
/// every port-map entry removed across all cascade levels.
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

/// One level of a cascading port-map removal.
pub struct RemovedPortMapLevel {
    /// The group whose own port-map entry was removed at this level.
    pub group_id: Uuid,
    /// The external name `group_id` exposed the port under.
    pub external_port_name: String,
    /// The node the removed entry pointed at.
    pub internal_node_id: Uuid,
    /// Whether the mapping exposed an input or an output port.
    pub port_type: PortType,
}

/// Everything torn down by [`remove_port_map_cascade`], innermost level first.
pub struct PortMapCascadeRemoval {
    /// Structural information about each removed port mapping level for GUI responses.
    pub levels: Vec<RemovedPortMapLevel>,
    /// Connections disconnected where the cascade terminated.
    pub disconnected_connections: Vec<(Uuid, ConnectInfo)>,
    /// Unified composite delta to restore all mappings and edges on undo.
    pub delta: GraphDelta,
}

/// Disconnects every outward-exposed port-map chain for `node_id` in `parent_group_id`.
///
/// # Errors
/// Returns an error if querying or removing any cascade step fails.
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

/// Removes `group_id`'s port mapping for `external_port_name`, cascading outward
/// through all parent groups until reaching a live connection or an unused chain.
///
/// Returns `Ok(None)` if no mapping exists under that name.
///
/// # Errors
/// Returns an error if any group resolution, port unmapping, or disconnection step fails.
pub fn remove_port_map_cascade(
    scenery: &mut NodeGroup,
    group_id: Uuid,
    external_port_name: &str,
    port_type: PortType,
) -> OpmResult<Option<PortMapCascadeRemoval>> {
    let mut levels = Vec::new();
    let mut disconnected_connections = Vec::new();
    let mut port_deltas = Vec::new();
    let mut edge_deltas = Vec::new();

    let mut cur_group = group_id;
    let mut cur_name = external_port_name.to_string();

    loop {
        let (internal_node_id, _internal_port_name) =
            match scenery.with_group_node_mut(cur_group, |g| {
                let hit = g.graph().port_map(&port_type).get(&cur_name).cloned();
                if hit.is_some() {
                    let delta = g.remove_mapped_port(&cur_name, port_type)?;
                    Ok::<_, OpossumError>(Some((hit.unwrap(), delta)))
                } else {
                    Ok(None)
                }
            })?? {
                Some(((id, name), delta)) => {
                    port_deltas.push(delta);
                    (id, name)
                }
                None => {
                    if levels.is_empty() {
                        return Ok(None);
                    }
                    return Err(OpossumError::Other(
                        "chained port mapping vanished mid-cascade".into(),
                    ));
                }
            };

        let root_id = scenery.node_attr().uuid();
        if cur_group == root_id {
            levels.push(RemovedPortMapLevel {
                group_id: cur_group,
                external_port_name: cur_name,
                internal_node_id,
                port_type,
            });
            break;
        }

        let (_, parent_id) = scenery.node_recursive(cur_group)?;
        levels.push(RemovedPortMapLevel {
            group_id: cur_group,
            external_port_name: cur_name.clone(),
            internal_node_id,
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

            let mut deltas = Vec::new();
            for c in &connections {
                let delta = g.disconnect_nodes(c.src_uuid(), c.src_port())?;
                deltas.push(delta);
            }
            Ok::<_, OpossumError>((connections, deltas))
        })??;

        if !live_connections.0.is_empty() {
            edge_deltas.extend(live_connections.1);
            disconnected_connections.extend(live_connections.0.into_iter().map(|c| (parent_id, c)));
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

    // Assemble forward chronological delta sequence:
    // [edge_deltas..., outermost_port_delta, ..., innermost_port_delta]
    // When undone in reverse (iter().rev()), innermost ports are mapped first,
    // outer ports second, and terminal edges are reconnected last.
    let mut forward_deltas = edge_deltas;
    forward_deltas.extend(port_deltas.into_iter().rev());

    Ok(Some(PortMapCascadeRemoval {
        levels,
        disconnected_connections,
        delta: GraphDelta::Composite(forward_deltas),
    }))
}
