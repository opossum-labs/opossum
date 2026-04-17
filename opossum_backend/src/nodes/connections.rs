use actix_web::{
    HttpResponse, delete, get, patch, post,
    web::{self, Json},
};
use opossum_core::{
    meter,
    types::api_types::{ConnectInfo, ErrorResponse},
};
use serde::Deserialize;
use uom::si::length::meter;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

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
        (status = OK, description = "all connections of the group", body= Vec<ConnectInfo>, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found or not a group node", content_type="application/json")
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
    let connections = scenery.with_group_node(uuid, opossum_core::nodes::NodeGroup::connections)?;
    
    let connect_infos = connections
        .iter()
        .map(|c| {
            let is_reference = scenery
                .with_node_attr(c.target_id, |node_attr| {
                    let prop = node_attr.properties();
                    prop.get("reference id").is_ok()
                })
                .unwrap_or(false);
                
            ConnectInfo::new(
                c.src_id,
                c.src_port.clone(),
                c.target_id,
                c.target_port.clone(),
                c.distance.get::<meter>(),
                is_reference,
            )
        })
        .collect::<Vec<ConnectInfo>>();
        
    Ok(Json(connect_infos))
}

/// Connect two nodes
///
/// Connect two optical nodes by the given connection info.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"), // <-- Fehlte!
    ),
    request_body = ConnectInfo,
    responses(
        (status = CREATED, description = "node connection created", body = ConnectInfo, content_type="application/json"), // <-- 201 Created
        (status = BAD_REQUEST, body = ErrorResponse, description = "group UUID not found", content_type="application/json")
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
    
    document
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
        
    let is_ref_node = document
        .scenery()
        .with_node_attr(connect_info.target_uuid(), |n| {
            n.properties().get("reference id").is_ok()
        })?;
        
    let mut connect_info = connect_info.into_inner();
    connect_info.set_is_reference(is_ref_node);
    drop(document);
    
    Ok(HttpResponse::Created().json(connect_info)) // <-- REST Standard
}

/// Update a connection distance
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"), // <-- Fehlte!
    ),
    request_body = ConnectInfo,
    responses(
        (status = OK, description = "node connection updated", body = ConnectInfo, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "group UUID not found", content_type="application/json")
))]
#[patch("/{uuid}/connections")]
pub async fn update_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    connect_info: Json<ConnectInfo>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let mut document = data.document.lock();
    
    document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.update_connection_distance(
                connect_info.src_uuid(),
                connect_info.src_port(),
                meter!(connect_info.distance()),
            )
        })??;
        
    drop(document);
    Ok(HttpResponse::Ok().json(connect_info.into_inner()))
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
        (status = BAD_REQUEST, body = ErrorResponse, description = "group UUID not found or disconnection failed", content_type="application/json")
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
    document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.disconnect_nodes(query.src_uuid, &query.src_port)
        })??;
        
    drop(document);
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod test {
    use super::*;
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

    #[actix_web::test]
    async fn test_delete_connection_invalid_uuid() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(delete_connection)).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/{}/connections?src_uuid={}&src_port=out", Uuid::new_v4(), Uuid::new_v4()))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}