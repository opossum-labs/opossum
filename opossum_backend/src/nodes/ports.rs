use crate::{app_state::AppState, error::BackEndErrorResponse};
use actix_web::{HttpResponse, get, web};
use opossum_core::{
    core_optics::{OpticNode, PortType, node_attr::HasNodeAttr},
    types::api_types::{ErrorResponse, NodePortsResponse},
};
use uuid::Uuid;

/// Get the port names of an optical node
///
/// Note: If the node is inverted, the physical inputs and outputs are automatically swapped in the response.
#[utoipa::path(
    tag = "node",
    params(("uuid" = Uuid, Path, description = "UUID of the node")),
    responses(
        (status = OK, description = "Port names retrieved", body = NodePortsResponse, content_type="application/json"),
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

    // `node_recursive` only searches for `uuid` as a child inside the scenery graph, so it can
    // never find the scenery root's own uuid - special-case it the same way
    // `NodeGroup::with_group_node`/`with_group_node_mut` already do.
    //
    // Must dispatch through the polymorphic `OpticNode::ports()`, not read `NodeAttr::raw_ports()`
    // directly: `NodeGroup` overrides `ports()` to derive its exposed port list live from its own
    // port map, but `raw_ports()` (a separate, concrete field) is never kept in sync by
    // `map_input_port`/`map_output_port`/`remove_mapped_port` - so for any group with a port
    // mapping, `raw_ports()` is permanently stale/empty and this endpoint would 200 with nothing.
    let ports = if document.scenery().node_attr().uuid() == uuid {
        document.scenery().ports()
    } else {
        document.scenery().node_recursive(uuid)?.0.ports()
    };

    let response = NodePortsResponse {
        inputs: ports.ports(&PortType::Input).clone(),
        outputs: ports.ports(&PortType::Output).clone(),
    };

    Ok(HttpResponse::Ok().json(response)) // <-- Saubere Serialisierung
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

    /// Regression test for the bug where `GET /{uuid}/ports` 400'd when `uuid` was the scenery
    /// root's own id (e.g. refreshing a top-level group's ports after a cut+paste) because
    /// `node_recursive` only finds nodes nested *inside* the scenery, never the scenery itself.
    #[actix_web::test]
    async fn test_get_ports_of_scenery_root() {
        let app_state = create_test_state();
        let root_uuid = app_state.document.lock().scenery().node_attr().uuid();
        let app = test::init_service(App::new().app_data(app_state).service(get_ports)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/{root_uuid}/ports"))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    /// Regression test for the bug where `GET /{uuid}/ports` always reported an empty port list
    /// for a `NodeGroup` with a port mapping: the handler read `NodeAttr::raw_ports()` directly,
    /// which `map_input_port`/`map_output_port` never keep in sync - only the polymorphic
    /// `OpticNode::ports()` (which `NodeGroup` overrides to derive its exposed ports live from its
    /// own port map) reflects reality. Builds a group with a single node mapped to an external
    /// port and asserts the endpoint reports that port name, not an empty list.
    #[actix_web::test]
    async fn test_get_ports_of_group_with_mapped_port() {
        use opossum_core::nodes::{Dummy, NodeGroup};

        let app_state = create_test_state();
        let group_id = {
            let mut document = app_state.document.lock();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let node_a = group.add_node(Dummy::default()).unwrap();
            group
                .map_output_port(node_a, "output_1", "ext_out_1")
                .unwrap();
            scenery.add_node(group).unwrap()
        };

        let app = test::init_service(App::new().app_data(app_state).service(get_ports)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/{group_id}/ports"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let response: NodePortsResponse = test::read_body_json(resp).await;
        assert!(
            response.outputs.contains("ext_out_1"),
            "the group's mapped output port must be reported, not an empty list; got {:?}",
            response.outputs
        );
        assert!(
            response.inputs.is_empty(),
            "the group has no mapped input port"
        );
    }
}
