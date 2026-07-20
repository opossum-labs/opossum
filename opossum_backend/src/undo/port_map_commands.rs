//! `apply`/`describe` bodies for the group port-mapping [`Command`] variants: [`Command::AddPortMap`],
//! [`Command::RemovePortMap`].
use opossum_core::{
    error::OpossumError,
    opm_document::OpmDocument,
    prelude::PortType,
    types::api_types::{AddPortMappingRequest, ConnectInfo, DocumentChange, RemovePortMapQuery},
};
use uuid::Uuid;

use super::Command;
use crate::error::BackEndErrorResponse;

/// Exposes an internal node's port as an external port on `group_id`. `parent_group_id` (`group_id`'s own
/// parent) is carried only so undo/redo responses can tell the GUI which tab renders `group_id`'s node
/// box (and therefore its exposed-port list) - a group's ports are drawn on its node as shown in its
/// *parent's* tab, not inside the group's own tab.
#[derive(Clone)]
pub struct AddPortMap {
    pub group_id: Uuid,
    pub parent_group_id: Uuid,
    pub request: AddPortMappingRequest,
}

/// Removes an external port mapping from `group_id`, disconnecting anything wired to it first.
#[derive(Clone)]
pub struct RemovePortMap {
    pub group_id: Uuid,
    pub parent_group_id: Uuid,
    pub query: RemovePortMapQuery,
}

/// Exposes an internal node's port as an external port on `cmd.group_id`, returning the
/// [`Command::RemovePortMap`] that undoes it.
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group, or the internal node/port doesn't exist.
pub(super) fn apply_add_port_map(
    document: &mut OpmDocument,
    cmd: AddPortMap,
) -> Result<Command, BackEndErrorResponse> {
    let AddPortMap {
        group_id,
        parent_group_id,
        request,
    } = cmd;
    document
        .scenery_mut()
        .with_group_node_mut(group_id, |g| match request.port_type {
            PortType::Input => g.map_input_port(
                request.internal_node_id,
                &request.internal_port_name,
                &request.external_port_name,
            ),
            PortType::Output => g.map_output_port(
                request.internal_node_id,
                &request.internal_port_name,
                &request.external_port_name,
            ),
        })??;
    Ok(Command::RemovePortMap(RemovePortMap {
        group_id,
        parent_group_id,
        query: RemovePortMapQuery {
            external_port_name: request.external_port_name,
            port_type: request.port_type,
        },
    }))
}

/// Removes an external port mapping from `cmd.group_id`, disconnecting anything wired to it first.
/// Returns a [`Command::Batch`] of the [`Command::AddPortMap`] that recreates the mapping plus one
/// [`Command::AddEdge`] per connection that had to be torn down, since undoing the removal means
/// restoring both.
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group or `query.external_port_name` isn't
/// currently mapped.
pub(super) fn apply_remove_port_map(
    document: &mut OpmDocument,
    cmd: RemovePortMap,
) -> Result<Command, BackEndErrorResponse> {
    let RemovePortMap {
        group_id,
        parent_group_id,
        query,
    } = cmd;
    let RemovePortMapQuery {
        external_port_name,
        port_type,
    } = query;
    let (_, parent_group) = document.scenery().node_recursive(group_id)?;
    debug_assert_eq!(
        parent_group, parent_group_id,
        "captured parent_group_id must match the group's actual current parent"
    );

    // Capture the internal node/port this mapping pointed at, so the inverse can recreate it.
    let internal = document.scenery_mut().with_group_node_mut(group_id, |g| {
        g.graph()
            .port_map(&port_type)
            .get(&external_port_name)
            .cloned()
    })?;
    let Some((internal_node_id, internal_port_name)) = internal else {
        return Err(BackEndErrorResponse::new(
            400,
            "Opossum",
            &format!("Port mapping '{external_port_name}' not found"),
        ));
    };

    // Disconnect any external connections using this mapped port, capturing them for the inverse.
    let torn_down = document
        .scenery_mut()
        .with_group_node_mut(parent_group, |g| {
            let connections = g
                .graph()
                .get_connection_info_of_node(group_id)
                .iter()
                .map(|c| ConnectInfo::from_connection_info(c, false))
                .filter(|c| match port_type {
                    PortType::Output => {
                        c.src_uuid() == group_id && c.src_port() == external_port_name
                    }
                    PortType::Input => {
                        c.target_uuid() == group_id && c.target_port() == external_port_name
                    }
                })
                .collect::<Vec<_>>();
            for c in &connections {
                g.disconnect_nodes(c.src_uuid(), c.src_port())?;
            }
            Ok::<_, OpossumError>(connections)
        })??;

    document.scenery_mut().with_group_node_mut(group_id, |g| {
        g.remove_mapped_port(&external_port_name, port_type)
    })?;

    let mut inverse = vec![Command::AddPortMap(AddPortMap {
        group_id,
        parent_group_id,
        request: AddPortMappingRequest {
            internal_node_id,
            internal_port_name,
            external_port_name,
            port_type,
        },
    })];
    for connect_info in torn_down {
        inverse.push(Command::AddEdge {
            group_id: parent_group,
            connect_info,
        });
    }
    Ok(Command::Batch(inverse))
}

/// Describes the effect of a [`Command::AddPortMap`] or [`Command::RemovePortMap`] in the GUI-facing
/// [`DocumentChange`] shape. A port-map change touches two tabs, like `MoveNodes` touches two: the
/// group's own tab (where `mapped_ports` drives the "mapped" symbol on the internal node's port) and its
/// parent's tab (where the group's own exposed-port list is rendered on its node box).
pub(super) fn describe(group_id: &Uuid, parent_group_id: &Uuid) -> Vec<DocumentChange> {
    vec![
        DocumentChange::GraphNeedsRefresh {
            graph_id: *group_id,
        },
        DocumentChange::GraphNeedsRefresh {
            graph_id: *parent_group_id,
        },
    ]
}
