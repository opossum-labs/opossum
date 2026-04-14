use crate::{app_state::AppState, error::BackEndErrorResponse};
use actix_web::{
    delete, get, post,
    web::{self, Json},
};
use opossum_core::{
    prelude::{OpticNode, PortMap, PortType},
    types::api_types::ConnectInfo,
};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddPortMappingRequest {
    pub internal_node_id: Uuid,
    pub internal_port_name: String,
    pub external_port_name: String,
    pub port_type: PortType,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct RemovePortMapQuery {
    /// External port name of the group port mapping
    pub external_port_name: String,
    /// Type of the port (e.g., Input or Output)
    pub port_type: PortType,
}

/// Get the port mappings of a group node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose portmaps should be sent"),
    ),
    responses(
        (status = OK, description = "Node portmaps successfully sent!"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/port_mappings")]
pub async fn get_port_mappings(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<(PortMap, PortMap)>, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let port_maps = data
        .document
        .lock()
        .scenery_mut()
        .with_group_node_mut(group_id, |g| {
            (
                g.graph().port_map(&PortType::Input).clone(),
                g.graph().port_map(&PortType::Output).clone(),
            )
        })?;
    Ok(Json(port_maps))
}
/// Map a port of an internal node to a port of the group node, effectively exposing it as an external port of the group.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port shouldbe mapped to a port of an internal node"),
    ),
    request_body(content = String,
        description = "Node uuid of internal node, tuple of internal port name of node and external port name pof group and the port type",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Node port successfully mapped to group port"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/{uuid}/port_mappings")]
pub async fn post_port_mapping(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    port_mapping_request: web::Json<AddPortMappingRequest>,
) -> Result<Json<(Vec<String>, Vec<String>)>, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let pmap_inf = port_mapping_request.into_inner();
    let ports = data
        .document
        .lock()
        .scenery_mut()
        .with_group_node_mut(group_id, |g| {
            match pmap_inf.port_type {
                PortType::Input => g.map_input_port(
                    pmap_inf.internal_node_id,
                    &pmap_inf.internal_port_name,
                    &pmap_inf.external_port_name,
                ),
                PortType::Output => g.map_output_port(
                    pmap_inf.internal_node_id,
                    &pmap_inf.internal_port_name,
                    &pmap_inf.external_port_name,
                ),
            }?;
            let ports = g.ports();
            let inputs = ports
                .ports(&PortType::Input)
                .keys()
                .cloned()
                .collect::<Vec<String>>();
            let outputs = ports
                .ports(&PortType::Output)
                .keys()
                .cloned()
                .collect::<Vec<String>>();
            Ok::<(Vec<String>, Vec<String>), BackEndErrorResponse>((inputs, outputs))
        })??;

    Ok(Json(ports))
}
/// Remove a port mapping from a group
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port-map should be removed"),
        RemovePortMapQuery
    ),
    responses(
        (status = OK, description = "Node port successfully removed from group"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[delete("/{uuid}/port_mappings")]
pub async fn remove_port_map(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    query: web::Query<RemovePortMapQuery>,
) -> Result<Json<(bool, Vec<ConnectInfo>, Uuid)>, BackEndErrorResponse> {
    let group_id = path.into_inner();

    // Entpacke die Query-Parameter
    let RemovePortMapQuery {
        external_port_name,
        port_type,
    } = query.into_inner();

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

    // get parent of node
    let (_, parent_group) = scenery.node_recursive(group_id)?;

    // get connections
    let connections = scenery.with_group_node_mut(parent_group, |g| {
        let c = g.graph().get_connection_info_of_node(group_id);

        // does not matter if it is a reference, as the connections are just removed
        c.iter()
            .map(|c| ConnectInfo::from_connection_info(c, false))
            .collect::<Vec<ConnectInfo>>()
    })?;

    // remove connections first before removing the mapping
    scenery.with_group_node_mut(parent_group, |g| {
        for c in &connections {
            g.disconnect_nodes(c.src_uuid(), c.src_port())?;
        }
        Ok::<(), BackEndErrorResponse>(())
    })??;

    let port_removed = scenery.with_group_node_mut(group_id, |g| {
        g.remove_mapped_port(&external_port_name, port_type)
    })?;

    Ok(Json((port_removed, connections, parent_group)))
}
