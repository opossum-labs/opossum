use crate::{app_state::AppState, error::BackEndErrorResponse};
use actix_web::{HttpResponse, get, patch, web};
use opossum_core::{
    core_optics::PortType,
    error::OpossumError, // <-- Hinzugefügt für das saubere Error-Handling
    types::api_types::{ErrorResponse, NodePortsResponse, UpdatePortRequest},
    utils::LockExt,
};
use uuid::Uuid;

/// Get all port configurations of an optical node
///
/// Returns the port configurations (Aperture, Coating, LIDT).
/// Note: If the node is inverted, the physical inputs and outputs are automatically swapped in the response.
#[utoipa::path(
    tag = "node",
    params(("uuid" = Uuid, Path, description = "UUID of the node")),
    responses(
        (status = OK, description = "Port configurations retrieved", body = NodePortsResponse, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found")
    )
)]
#[get("/{uuid}/ports")]
pub async fn get_ports(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    // <-- Konsistente HttpResponse
    let uuid = path.into_inner();
    let document = data.document.lock();

    let node_attr = document
        .scenery()
        .node_recursive(uuid)?
        .0
        .optical_ref
        .lock_opm()?
        .node_attr()
        .clone();

    let ports = node_attr.raw_ports();

    let response = NodePortsResponse {
        inputs: ports.ports(&PortType::Input).clone(),
        outputs: ports.ports(&PortType::Output).clone(),
    };

    Ok(HttpResponse::Ok().json(response)) // <-- Saubere Serialisierung
}

/// Update a specific port configuration (Aperture, Coating, LIDT)
///
/// Modifies only the provided properties of a port. Omitted fields remain unchanged.
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the node"),
        ("port_type" = PortType, Path, description = "Type of the port (Input or Output)"),
        ("port_name" = String, Path, description = "Name of the port (e.g. 'input_1')")
    ),
    request_body(
        content = UpdatePortRequest,
        description = "The properties to update",
        content_type = "application/json"
    ),
    responses(
        (status = NO_CONTENT, description = "Port successfully updated"), // <-- NO_CONTENT!
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID or Port not found")
    )
)]
#[patch("/{uuid}/ports/{port_type}/{port_name}")]
pub async fn patch_port(
    data: web::Data<AppState>,
    path: web::Path<(Uuid, PortType, String)>,
    update: web::Json<UpdatePortRequest>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let (uuid, port_type, port_name) = path.into_inner();
    let update_data = update.into_inner();

    data.document
        .lock()
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            let port_map = node_attr.raw_ports_mut().ports_mut(&port_type);

            if let Some(port) = port_map.get_mut(&port_name) {
                if let Some(new_aperture) = update_data.aperture {
                    port.aperture = new_aperture;
                }
                if let Some(new_coating) = update_data.coating {
                    port.coating = new_coating;
                }
                if let Some(new_lidt) = update_data.lidt {
                    port.lidt = new_lidt;
                }
                Ok::<(), OpossumError>(()) // <-- Expliziter Typ für den Compiler
            } else {
                Err(OpossumError::Other(format!(
                    "{port_type} port '{port_name}' not found"
                )))
            }
        })??;

    Ok(HttpResponse::NoContent().finish()) // <-- REST-konformer Abschluss
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};

    fn create_test_state() -> Data<AppState> {
        Data::new(AppState::default())
    }

    #[actix_web::test]
    async fn test_get_ports_invalid_uuid() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(get_ports)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/{}/ports", Uuid::new_v4()))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_patch_port_invalid_uuid() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(patch_port)).await;

        let update_req = UpdatePortRequest {
            aperture: None,
            coating: None,
            lidt: None,
        };

        let req = test::TestRequest::patch()
            .uri(&format!("/{}/ports/Input/input_1", Uuid::new_v4()))
            .set_json(&update_req)
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
