use actix_web::{
    HttpResponse, delete, get, post,
    web::{self},
};
use opossum_core::{
    error::OpossumError,
    nodes::GraphDelta,
    prelude::{OpticNode, PortType},
    types::api_types::{
        AddPortMappingRequest, ErrorResponse, PortMappingsResponse, PortNamesResponse,
        RemovePortMapQuery, RemovePortMapResponse,
    },
};
use uuid::Uuid;

use crate::{
    app_state::AppState, error::BackEndErrorResponse, helper_functions::remove_port_map_cascade,
    undo::Command,
};

/// Get the port mappings of a group node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose portmaps should be sent"),
    ),
    responses(
        (status = OK, description = "Node portmaps successfully sent!", body = PortMappingsResponse, content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[get("/{uuid}/port_mappings")]
pub async fn get_port_mappings(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let (inputs, outputs) =
        data.document
            .lock()
            .scenery_mut()
            .with_group_node_mut(group_id, |g| {
                (
                    g.graph().port_map(&PortType::Input).clone(),
                    g.graph().port_map(&PortType::Output).clone(),
                )
            })?;
    let response = PortMappingsResponse { inputs, outputs };
    Ok(HttpResponse::Ok().json(response))
}

/// Map a port of an internal node to a port of the group node.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port should be mapped"),
    ),
    request_body = AddPortMappingRequest,
    responses(
        (status = CREATED, description = "Node port successfully mapped", body = PortNamesResponse, content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[post("/{uuid}/port_mappings")]
pub async fn post_port_mapping(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    port_mapping_request: web::Json<AddPortMappingRequest>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let pmap_inf = port_mapping_request.into_inner();

    let (inputs, outputs, delta) =
        data.document
            .lock()
            .scenery_mut()
            .with_group_node_mut(group_id, |g| {
                let delta = match pmap_inf.port_type {
                    PortType::Input => g.map_input_port(
                        pmap_inf.internal_node_id,
                        &pmap_inf.internal_port_name,
                        &pmap_inf.external_port_name,
                    )?,
                    PortType::Output => g.map_output_port(
                        pmap_inf.internal_node_id,
                        &pmap_inf.internal_port_name,
                        &pmap_inf.external_port_name,
                    )?,
                };
                let ports = g.ports();
                let inputs: Vec<String> = ports.ports(&PortType::Input).keys().cloned().collect();
                let outputs: Vec<String> = ports.ports(&PortType::Output).keys().cloned().collect();
                Ok::<(Vec<String>, Vec<String>, GraphDelta), OpossumError>((inputs, outputs, delta))
            })??;

    data.push_undo(Command::UndoGraph(Box::new(delta)));

    let response = PortNamesResponse { inputs, outputs };
    Ok(HttpResponse::Created().json(response))
}

/// Remove a port mapping from a group
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port-map should be removed"),
        RemovePortMapQuery
    ),
    responses(
        (status = OK, description = "Node port successfully removed", body = RemovePortMapResponse, content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[delete("/{uuid}/port_mappings")]
pub async fn remove_port_map(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    query: web::Query<RemovePortMapQuery>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_id = path.into_inner();
    let RemovePortMapQuery {
        external_port_name,
        port_type,
    } = query.into_inner();
    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

    let Some(cascade) = remove_port_map_cascade(scenery, group_id, &external_port_name, port_type)?
    else {
        let response = RemovePortMapResponse {
            port_removed: false,
            removed_port_mappings: Vec::new(),
            disconnected_connections: Vec::new(),
        };
        return Ok(HttpResponse::Ok().json(response));
    };

    // Store the composite graph delta directly on the undo stack
    data.push_undo(Command::UndoGraph(Box::new(cascade.delta)));

    let removed_port_mappings = cascade
        .levels
        .into_iter()
        .map(|level| {
            (
                level.group_id,
                level.internal_node_id,
                level.external_port_name,
                level.port_type,
            )
        })
        .collect();

    let response = RemovePortMapResponse {
        port_removed: true,
        removed_port_mappings,
        disconnected_connections: cascade.disconnected_connections,
    };
    Ok(HttpResponse::Ok().json(response))
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};

    fn create_test_state() -> Data<AppState> {
        Data::new(AppState::default())
    }

    #[actix_web::test]
    async fn test_get_port_mappings_invalid_uuid() {
        let app_state = create_test_state();
        let app =
            test::init_service(App::new().app_data(app_state).service(get_port_mappings)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/{}/port_mappings", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_remove_port_map_invalid_uuid() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(remove_port_map)).await;
        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{}/port_mappings?external_port_name=out&port_type=Output",
                Uuid::new_v4()
            ))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
