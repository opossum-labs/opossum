use crate::{app_state::AppState, error::ErrorResponse};
use actix_web::{
    delete, get, post, put,
    web::{self, Data, Json},
};
use opossum::{meter, nodes::create_node_ref, optic_ref::OpticRef};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

/// Add a new node to the toplevel scenery
#[utoipa::path(tag = "node",
    request_body(content = String,
        description = "type node the optical node to be created",
        content_type = "text/plain",
        example ="dummy"
    ),
    responses(
        (status = OK, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Node of the given type not found", content_type="application/json")
    )
)]
#[post("/nodes")]
async fn add_node(
    data: web::Data<AppState>,
    node_type: String,
) -> Result<Json<OpticRef>, ErrorResponse> {
    let node_ref = create_node_ref(&node_type)?;
    let mut scenery = data.scenery.lock().unwrap();
    scenery.add_node_ref(node_ref.clone())?;
    drop(scenery);
    Ok(web::Json(node_ref))
}
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    )
)]
/// Get all node properties
#[get("/nodes/{uuid}")]
async fn get_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<OpticRef>, ErrorResponse> {
    let uuid = path.into_inner();
    let scenery = data.scenery.lock().unwrap();
    let node_ref = scenery.node(uuid)?;
    drop(scenery);
    Ok(web::Json(node_ref))
}
/// Update node properties
#[utoipa::path(tag = "node")]
#[put("/nodes/{uuid}")]
async fn put_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<OpticRef>, ErrorResponse> {
    let uuid = path.into_inner();
    let scenery = data.scenery.lock().unwrap();
    let node_ref = scenery.node(uuid)?;
    drop(scenery);
    Ok(web::Json(node_ref))
}
/// Delete a node
#[utoipa::path(tag = "node")]
#[delete("/nodes/{uuid}")]
async fn delete_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, ErrorResponse> {
    let uuid = path.into_inner();
    let mut scenery = data.scenery.lock().unwrap();
    let deleted_nodes = scenery.delete_node(uuid)?;
    drop(scenery);
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
    data.scenery.lock().unwrap().connect_nodes(
        connect_info.src_uuid,
        &connect_info.src_port,
        connect_info.target_uuid,
        &connect_info.target_port,
        meter!(connect_info.distance),
    )?;
    Ok(connect_info)
}
/// Disconnect two nodes
#[utoipa::path(tag = "node")]
#[delete("/connection")]
async fn delete_connection(
    data: web::Data<AppState>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, ErrorResponse> {
    data.scenery
        .lock()
        .unwrap()
        .disconnect_nodes(connect_info.src_uuid, &connect_info.src_port)?;
    Ok(connect_info)
}

pub fn configure(store: Data<AppState>) -> impl FnOnce(&mut ServiceConfig<'_>) {
    |config: &mut ServiceConfig<'_>| {
        config
            .app_data(store)
            .service(add_node)
            .service(get_node)
            .service(put_node)
            .service(delete_node)
            .service(post_connection)
            .service(delete_connection);
    }
}
