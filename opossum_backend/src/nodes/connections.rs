use actix_web::{
    HttpResponse, delete, get, patch, post,
    web::{self, Json},
};
use opossum_core::{
    meter,
    nodes::NodeGroup,
    types::api_types::{ConnectInfo, ErrorResponse, UpdateConnectionRequest},
};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{connect_infos, is_reference_target},
    undo::Command,
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteConnectionQuery {
    /// UUID of the source node
    pub src_uuid: Uuid,
    /// Name of the source port
    pub src_port: String,
}

#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "all connections of the group", body = Vec<ConnectInfo>, content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found or not a group node", content_type = "application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[get("/{uuid}/connections")]
pub async fn get_connections(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<ConnectInfo>>, BackEndErrorResponse> {
    let document = data.document.lock();
    let scenery = document.scenery();
    let uuid = path.into_inner();
    let connections = scenery.with_group_node(uuid, NodeGroup::connections)?;
    Ok(Json(connect_infos(scenery, &connections)))
}

/// Connect two nodes
///
/// Connect two optical nodes by the given connection info.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    request_body = ConnectInfo,
    responses(
        (status = CREATED, description = "node connection created", body = ConnectInfo, content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "group UUID not found", content_type = "application/json")
    )
)]
#[post("/{uuid}/connections")]
pub async fn post_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    connect_info: Json<ConnectInfo>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let mut document = data.document.lock();

    // The core's connect_nodes returns GraphDelta::NodesConnected directly
    let delta = document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.connect_nodes(
                connect_info.src_uuid(),
                connect_info.src_port(),
                connect_info.target_uuid(),
                connect_info.target_port(),
                meter!(connect_info.distance()),
            )
        })??;

    let is_ref_node = is_reference_target(document.scenery(), connect_info.target_uuid());
    let mut connect_info = connect_info.into_inner();
    connect_info.set_is_reference(is_ref_node);

    data.push_undo(Command::UndoGraph(Box::new(delta)));
    drop(document);

    Ok(HttpResponse::Created().json(connect_info))
}

/// Update a connection distance
///
/// Updates only the geometric distance of an existing connection identified by its source node and port.
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    request_body = UpdateConnectionRequest,
    responses(
        (status = NO_CONTENT, description = "Node connection successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Group UUID not found or connection invalid", content_type = "application/json")
    )
)]
#[patch("/{uuid}/connections")]
pub async fn update_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req_body: Json<UpdateConnectionRequest>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let update_req = req_body.into_inner();
    let mut document = data.document.lock();

    // Core captures old and new distance inside GraphDelta::ConnectionDistanceChanged
    let delta = document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.update_connection_distance(
                update_req.src_uuid,
                &update_req.src_port,
                meter!(update_req.distance),
            )
        })??;

    data.push_undo(Command::UndoGraph(Box::new(delta)));
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

/// Disconnect two nodes
///
/// Removes the connection originating from the specified source node and port.
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group containing the connection"),
        DeleteConnectionQuery
    ),
    responses(
        (status = NO_CONTENT, description = "node connection successfully deleted"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "group UUID not found or disconnection failed", content_type = "application/json")
    )
)]
#[delete("/{uuid}/connections")]
pub async fn delete_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteConnectionQuery>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let query = query.into_inner();
    let mut document = data.document.lock();

    // Core captures connection details inside GraphDelta::NodesDisconnected
    let delta = document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.disconnect_nodes(query.src_uuid, &query.src_port)
        })??;

    data.push_undo(Command::UndoGraph(Box::new(delta)));
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::document::test::state_with_a_source_in_front_of_a_reference;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};

    fn create_test_state() -> Data<AppState> {
        Data::new(AppState::default())
    }

    #[actix_web::test]
    async fn test_get_connections_invalid_uuid() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(get_connections)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/{}/connections", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// Every connection of a group is listed with its distance in meters, marked when it leads
    /// into a reference.
    #[actix_web::test]
    async fn the_connections_of_a_group_are_listed() {
        let (app_state, root, _) = state_with_a_source_in_front_of_a_reference();
        let app = test::init_service(App::new().app_data(app_state).service(get_connections)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/{root}/connections"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let listed: Vec<ConnectInfo> = test::read_body_json(resp).await;
        let mut seen: Vec<(bool, f64)> = listed
            .iter()
            .map(|c| (c.targets_reference(), c.distance()))
            .collect();
        seen.sort_by_key(|(into_reference, _)| *into_reference);
        assert_eq!(seen.len(), 2);
        assert!(!seen[0].0 && (seen[0].1 - 0.1).abs() < 1e-12, "{seen:?}");
        assert!(seen[1].0 && (seen[1].1 - 0.05).abs() < 1e-12, "{seen:?}");
    }

    #[actix_web::test]
    async fn test_delete_connection_invalid_uuid() {
        let app_state = create_test_state();
        let app =
            test::init_service(App::new().app_data(app_state).service(delete_connection)).await;
        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{}/connections?src_uuid={}&src_port=out",
                Uuid::new_v4(),
                Uuid::new_v4()
            ))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
