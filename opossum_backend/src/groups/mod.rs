use actix_web::{post, web::{self, Json, PathConfig}};
use opossum_core::types::api_types::{ConnectInfo, NodeInfo};
use uuid::Uuid;
use utoipa_actix_web::service_config::ServiceConfig;

use crate::{app_state::AppState, error::BackEndErrorResponse, groups::helper_functions::{add_converted_group_to_scenery, build_new_group, build_reference_map, collect_group_connections, collect_node_refs_and_pos, create_new_group_node_info, split_connections}};

mod helper_functions;



/// Convert a set of nodes to a group node by creating a new group node and instering the nodes
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

    let (node_refs, pos) = collect_node_refs_and_pos(&data, &nodes_to_convert);
    let all_connections = collect_group_connections(&data, group_id)?;

    let reference_map = build_reference_map(&data, &all_connections);

    let (inside_connections, map_input_connections, map_output_connections) =
        split_connections(&all_connections, &reference_map, &nodes_to_convert);

    let new_group = build_new_group(
        node_refs,
        &inside_connections,
        &map_input_connections,
        &map_output_connections,
    )?;

    let new_group_id = add_converted_group_to_scenery(
        &data,
        group_id,
        nodes_to_convert,
        new_group,
        &map_input_connections,
        &map_output_connections,
    )?;

    let new_group_node_info = create_new_group_node_info(&data, new_group_id, pos)?;

    let mut all_external_connections = map_input_connections;
    all_external_connections.extend(map_output_connections);

    Ok(Json((new_group_node_info, all_external_connections)))
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(post_convert_nodes_to_group);
    cfg.app_data(PathConfig::default().error_handler(|err, _req| {
        BackEndErrorResponse::new(400, "parse error", &err.to_string()).into()
    }));
}
