use crate::{app_state::AppState, error::ErrorResponse};
use actix_web::{
    delete, get, post, put,
    web::{self, Json},
};
use opossum::{meter, nodes::create_node_ref, optic_ref::OpticRef};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

#[derive(Serialize, Deserialize, ToSchema)]
struct NodeInfo {
    uuid: Uuid,
    name: String,
    node_type: String,
}
/// Return a list of nodes in the toplevel scenery
#[utoipa::path(tag = "node",
responses(
    (status = OK, body= Vec<NodeInfo>, description = "successful", content_type="application/json"),
)
)]
#[get("/nodes")]
async fn get_nodes(data: web::Data<AppState>) -> Result<Json<Vec<NodeInfo>>, ErrorResponse> {
    let document = data.document.lock().unwrap();
    let scenery = document.scenery().clone();
    drop(document);
    let nodes_info: Vec<NodeInfo> = scenery
        .nodes()
        .iter()
        .map(|n| {
            let node = n.optical_ref.lock().unwrap();
            let name = node.name();
            let node_type = node.node_type();
            drop(node);
            NodeInfo {
                uuid: n.uuid(),
                name,
                node_type,
            }
        })
        .collect();
    Ok(Json(nodes_info))
}
/// Add a new node to the toplevel scenery
#[utoipa::path(tag = "node",
    request_body(content = String,
        description = "type node the optical node to be created",
        content_type = "text/plain",
        example ="dummy"
    ),
    responses(
        (status = OK, body= NodeInfo, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Node of the given type not found", content_type="application/json")
    )
)]
#[post("/nodes")]
async fn post_node(
    data: web::Data<AppState>,
    node_type: String,
) -> Result<Json<NodeInfo>, ErrorResponse> {
    let node_ref = create_node_ref(&node_type)?;
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    scenery.add_node_ref(node_ref.clone())?;
    drop(document);
    let node = node_ref.optical_ref.lock().unwrap();
    let name = node.name();
    let node_type = node.node_type();
    drop(node);
    let node_info = NodeInfo {
        uuid: node_ref.uuid(),
        name,
        node_type,
    };
    Ok(Json(node_info))
}
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    responses(
        (status = OK, description = "get all node properties", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
/// Get all node properties
#[get("/nodes/{uuid}")]
async fn get_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<OpticRef>, ErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock().unwrap();
    let node_ref = document.scenery().node(uuid)?;
    drop(document);
    Ok(web::Json(node_ref))
}
/// Update node properties
#[utoipa::path(tag = "node",
    responses(
        (status = OK, description = "node properties updated", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[put("/nodes/{uuid}")]
async fn put_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<OpticRef>, ErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    let _node_ref = scenery.node(uuid)?;
    drop(document);
    todo!();
    // Ok(web::Json(node_ref))
}
/// Delete a node
///
/// This function deletes a node node. It also deletes reference nodes which refer to this node.
/// A list of UUIDs of the effectively deleted nodes is returned.
#[utoipa::path(tag = "node",
responses(
    (status = OK, body= Vec<Uuid>, description = "UUIDs of the deleted nodes", content_type="application/json"),
    (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
))]
#[delete("/nodes/{uuid}")]
async fn delete_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, ErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    let deleted_nodes = scenery.delete_node(uuid)?;
    drop(document);
    Ok(web::Json(deleted_nodes))
}
/// Connection Information
#[derive(ToSchema, Serialize, Deserialize)]
struct ConnectInfo {
    /// UUID of the source node
    src_uuid: Uuid,
    /// name of the (outgoing) source port
    src_port: String,
    /// UUID of the target node
    target_uuid: Uuid,
    /// name of the (incoming) target port
    target_port: String,
    /// geometric distance between nodes (optical axis) in meters.
    distance: f64,
}
/// Connect two nodes
///
/// Connect to optical nodes by the given connection info.
#[utoipa::path(tag = "node")]
#[post("/connection")]
async fn post_connection(
    data: web::Data<AppState>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, ErrorResponse> {
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    scenery.connect_nodes(
        connect_info.src_uuid,
        &connect_info.src_port,
        connect_info.target_uuid,
        &connect_info.target_port,
        meter!(connect_info.distance),
    )?;
    drop(document);
    Ok(connect_info)
}
/// Disconnect two nodes
#[utoipa::path(tag = "node")]
#[delete("/connection")]
async fn delete_connection(
    data: web::Data<AppState>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, ErrorResponse> {
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    scenery.disconnect_nodes(connect_info.src_uuid, &connect_info.src_port)?;
    drop(document);
    Ok(connect_info)
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_nodes);
    cfg.service(post_node);
    cfg.service(get_node);
    cfg.service(put_node);
    cfg.service(delete_node);
    cfg.service(post_connection);
    cfg.service(delete_connection);
}
