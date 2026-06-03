use actix_web::{
    HttpResponse, delete, get, post,
    web::{self},
};
use opossum_core::{
    error::OpossumError,
    prelude::{OpticNode, PortType},
    types::api_types::{
        AddPortMappingRequest, ConnectInfo, ErrorResponse, PortMappingsResponse, PortNamesResponse,
        RemovePortMapQuery, RemovePortMapResponse,
    },
};
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

/// Get the port mappings of a group node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose portmaps should be sent"),
    ),
    responses(
        (status = OK, description = "Node portmaps successfully sent!", body = PortMappingsResponse, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
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
///
/// This will create a new port on the group node and connect it to the internal node's port. The new port will be named as specified in the request.
/// If a port with the same name already exists on the group node, an error will be returned. This function will also return the updated lists of mapped
/// input and output ports of the group node, which can be used to update the UI accordingly.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port should be mapped"),
    ),
    request_body = AddPortMappingRequest,
    responses(
        (status = CREATED, description = "Node port successfully mapped", body = PortNamesResponse, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
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

    let (inputs, outputs) =
        data.document
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
                let inputs: Vec<String> = ports.ports(&PortType::Input).keys().cloned().collect();
                let outputs: Vec<String> = ports.ports(&PortType::Output).keys().cloned().collect();

                Ok::<(Vec<String>, Vec<String>), OpossumError>((inputs, outputs)) // <-- OpossumError statt BackEndErrorResponse!
            })??;

    let response = PortNamesResponse { inputs, outputs };
    Ok(HttpResponse::Created().json(response)) // 201 Created
}

/// Remove a port mapping from a group
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Uuid of a group whose port-map should be removed"),
        RemovePortMapQuery
    ),
    responses(
        (status = OK, description = "Node port successfully removed", body = RemovePortMapResponse, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
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

    let (_, parent_group) = scenery.node_recursive(group_id)?;

    let connections = scenery.with_group_node_mut(parent_group, |g| {
        let c = g.graph().get_connection_info_of_node(group_id);
        c.iter()
            .map(|c| ConnectInfo::from_connection_info(c, false))
            .collect::<Vec<ConnectInfo>>()
    })?;

    // Disconnect (idiomatisches Error-Handling mit OpossumError)
    scenery.with_group_node_mut(parent_group, |g| {
        for c in &connections {
            g.disconnect_nodes(c.src_uuid(), c.src_port())?;
        }
        Ok::<(), OpossumError>(())
    })??;

    let port_removed = scenery.with_group_node_mut(group_id, |g| {
        g.remove_mapped_port(&external_port_name, port_type)
    })?;

    let response = RemovePortMapResponse {
        port_removed,
        connections,
        parent_group_uuid: parent_group,
    };

    Ok(HttpResponse::Ok().json(response)) // 200 OK (Daten werden zurückgegeben)
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
