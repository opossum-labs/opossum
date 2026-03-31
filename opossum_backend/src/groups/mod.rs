use actix_web::{
    delete, post, get, web::{self, Json, PathConfig}
};
use opossum_core::{ prelude::{OpticNode, PortMap, PortType}, types::api_types::{ConnectInfo, NodeInfo}};
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    groups::helper_functions::{
        add_converted_group_to_scenery, build_new_group_from_refs_and_conns,
        collect_group_connections, collect_node_refs_and_pos, connect_from_info,
        create_new_group_node_info, split_sort_connections,
    },
};

mod helper_functions;

/// Convert the given nodes into a new subgroup within an existing group.
///
/// The source group is defined by the path parameter `uuid` (`group_id`).
/// The request body contains a list of node UUIDs that will be removed from
/// the source group and wrapped into a newly created group node.
///
/// The newly created group will preserve internal connections between the
/// selected nodes and expose relevant external connections.
///
/// # Arguments
///
/// * `uuid` (path) - The UUID of the group (`group_id`) that currently contains
///   the nodes to be converted.
/// * `Vec<Uuid>` (body) - A list of node UUIDs that should be converted into
///   a new group node.
///
/// # Returns
///
/// Returns a tuple containing:
/// - `NodeInfo`: Information about the newly created group node.
/// - `Vec<ConnectInfo>`: External connections to and from the new group node.
///
/// # Errors
///
/// This function will return an error if:
/// - The specified group was not found.
/// - Any of the provided node UUIDs are invalid.
/// - The group construction or insertion fails due to internal consistency constraints.
#[utoipa::path(tag = "group",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of the group in which the nodes are currently contained"),
    ),
    request_body(content = String,
        description = "Set of node uuids that correspond to the nodes that should be converted to a group",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Nodes successfully converted to group"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/{uuid}/convert_to_group")]
pub async fn post_convert_nodes_to_group(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    nodes_to_convert: web::Json<Vec<Uuid>>,
) -> Result<Json<(NodeInfo, Vec<ConnectInfo>)>, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let nodes_to_convert = nodes_to_convert.into_inner();

    //collect data
    let (node_refs, pos) = collect_node_refs_and_pos(&data, &nodes_to_convert);
    let all_connections = collect_group_connections(&data, group_id)?;
    let split = split_sort_connections(&data, &all_connections, &nodes_to_convert);

    //create new group: add nodes and connections
    let new_group = build_new_group_from_refs_and_conns(node_refs, &split)?;

    //addnew group to scenery
    let new_group_id = add_converted_group_to_scenery(
        &data,
        group_id,
        nodes_to_convert,
        new_group,
        &split.input,
        &split.output,
    )?;

    //create the nodeinfo struct for the GUI
    let new_group_node_info = create_new_group_node_info(&data, new_group_id, pos)?;

    let mut external_connections = split.input;
    external_connections.extend(split.output);

    Ok(Json((new_group_node_info, external_connections)))
}

/// Drop the given nodes from one group into another group.
///
/// The source group is defined by the path parameter `uuid` (`from_group_id`).
/// The request body contains a tuple:
/// - `Vec<Uuid>`: the list of node IDs to move
/// - `Uuid`: the target group ID (`drop_group_id`)
///
/// All specified nodes will be removed from the source group and inserted into
/// the target group, including their internal connections.
///
/// # Arguments
///
/// * `uuid` (path) - The UUID of the source group (`from_group_id`) from which
///   the nodes will be removed.
/// * `(Vec<Uuid>, Uuid)` (body) - A tuple where:
///     - The first element is the list of node UUIDs to move.
///     - The second element is the UUID of the destination group (`drop_group_id`).
///
/// # Errors
///
/// This function will return an error if:
/// - The source group or target group was not found.
/// - Any of the provided node UUIDs are invalid.
/// - The nodes cannot be removed or inserted due to internal consistency constraints.
#[utoipa::path(tag = "group",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of the group in which the nodes should be transferred"),
    ),
    request_body(content = String,
        description = "Set of node uuids that correspond to the nodes that should be transferred to a group",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Nodes successfully transferred to group"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/{uuid}/drop_into_group")]
pub async fn post_drop_nodes_into_group(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    nodes_to_drop_in_group: web::Json<(Vec<Uuid>, Uuid)>,
) -> Result<(), BackEndErrorResponse> {
    let from_group_id = path.into_inner();
    let (mut nodes_to_drop, drop_group_id) = nodes_to_drop_in_group.into_inner();

    //collect data
    let (node_refs, _) = collect_node_refs_and_pos(&data, &nodes_to_drop);
    let all_connections = collect_group_connections(&data, from_group_id)?;
    let split = split_sort_connections(&data, &all_connections, &nodes_to_drop);

    let mut document = data.document.lock();
    let scenery: &mut opossum_core::prelude::NodeGroup = document.scenery_mut();

    //delete nodes_to_drop from original scenery. Important to do this befor inserting the optic_refs as they will then also removed
    while let Some(node) = nodes_to_drop.pop() {
        let deleted = scenery.delete_node(node)?;
        for del_id in &deleted {
            nodes_to_drop.retain(|id| id != del_id);
        }
    }

    //add nodes_to_drop to group
    for node_ref in &node_refs {
        scenery.with_group_node_mut(drop_group_id, |g| g.add_node_ref(node_ref.clone()))??;
    }

    //connect nodes if there are any
    for conn in &split.inside {
        scenery.with_group_node_mut(drop_group_id, |g| connect_from_info(g, conn))??;
    }

    drop(document);
    Ok(())
}

#[utoipa::path(tag = "group",
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
#[post("/{uuid}/port_map")]
pub async fn add_port_map(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    port_map_info: web::Json<(Uuid, (String, String), PortType)>)
    -> Result<Json<(Vec<String>, Vec<String>)>, BackEndErrorResponse>{
    let group_id = path.into_inner();
    let (node_id_to_map, (internal_port_name, external_port_name), port_type) = port_map_info.into_inner();

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();
    
    let ports = scenery.with_group_node_mut(group_id, |g| {
        match port_type{
        PortType::Input => g.map_input_port(node_id_to_map, &internal_port_name, &external_port_name),
        PortType::Output => g.map_output_port(node_id_to_map, &internal_port_name, &external_port_name),
    }?;
    let ports = g.ports();
    let inputs = ports.ports(&PortType::Input).keys().cloned().collect::<Vec<String>>();
    let outputs = ports.ports(&PortType::Output).keys().cloned().collect::<Vec<String>>();
    Ok::<(Vec<String>, Vec<String>), BackEndErrorResponse>((inputs, outputs))
}
)??;


    Ok(Json(ports))
}

#[utoipa::path(tag = "group",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose portmaps should be sent"),
    ),
    responses(
        (status = OK, description = "Node portmaps successfully sent!"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/portmaps")]
pub async fn get_group_portmaps(
    data: web::Data<AppState>,
    path: web::Path<Uuid>
)
    -> Result<Json<(PortMap, PortMap)>, BackEndErrorResponse>{
    let group_id = path.into_inner();

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();
    
    let port_maps = scenery.with_group_node_mut(group_id, |g| {
        (g.graph().port_map(&PortType::Input).clone(),
        g.graph().port_map(&PortType::Output).clone())
}
)?;
    Ok(Json(port_maps))
}

#[utoipa::path(tag = "group",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose ports should be sent"),
    ),
    responses(
        (status = OK, description = "Node ports successfully sent!"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/ports")]
pub async fn get_group_ports(
    data: web::Data<AppState>,
    path: web::Path<Uuid>
)
    -> Result<Json<(Vec<String>, Vec<String>)>, BackEndErrorResponse>{
    let group_id = path.into_inner();

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();
    
let ports = scenery.with_group_node_mut(group_id, |g| {
    let ports = g.ports();
    let inputs = ports.ports(&PortType::Input).keys().cloned().collect::<Vec<String>>();
    let outputs = ports.ports(&PortType::Output).keys().cloned().collect::<Vec<String>>();
    Ok::<(Vec<String>, Vec<String>), BackEndErrorResponse>((inputs, outputs))
}
)??;
    Ok(Json(ports))
}



#[utoipa::path(tag = "group",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port-map should be removed "),
    ),
    request_body(content = String,
        description = "External port name of the group port mapping",
        content_type = "application/json",
    ),
    responses(
        (status = OK, description = "Node port successfully removed from group"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[delete("/{uuid}/port_map")]
pub async fn remove_port_map(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    external_port_name: web::Json<(String, PortType)>)
    -> Result<Json<(bool, Vec<ConnectInfo>, Uuid)>, BackEndErrorResponse>{
    let group_id = path.into_inner();
    let (external_port_name, port_type  ) = external_port_name.into_inner();

    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

    //get parent of node
    let (_, parent_group) = scenery.node_recursive(group_id)?;

    //get connections
    let connections = scenery.with_group_node_mut(parent_group, |g| {
        let c = g.graph().get_connection_info_of_node(group_id);

        //does not matter if it is a references, as the connections i just removed
        c.iter().map(|c| ConnectInfo::from_connection_info(c, false))    .collect::<Vec<ConnectInfo>>()
    })?;


    //remove connections first before removing the mapping
    scenery
        .with_group_node_mut(parent_group, |g| {
            for c in &connections{
                g.disconnect_nodes(c.src_uuid(), c.src_port())?
            }
            Ok::<(), BackEndErrorResponse>(())
        })??;
    
    let port_removed = scenery.with_group_node_mut(group_id, |g| {
        g.remove_mapped_port(&external_port_name, port_type)
    })?;
    Ok(Json((port_removed, connections, parent_group)))
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(post_convert_nodes_to_group);
    cfg.service(post_drop_nodes_into_group);
    cfg.service(add_port_map);
    cfg.service(remove_port_map);
    cfg.service(get_group_ports);
    cfg.service(get_group_portmaps);
    cfg.app_data(PathConfig::default().error_handler(|err, _req| {
        BackEndErrorResponse::new(400, "parse error", &err.to_string()).into()
    }));
}
