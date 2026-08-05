use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{apply_and_push_undo, parent_group_id_or_self},
    undo::{Command, PatchPort},
};
use actix_web::{HttpResponse, get, patch, web};
use opossum_core::{
    core_optics::{OpticNode, PortType, node_attr::HasNodeAttr},
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
        document
            .scenery()
            .node_recursive(uuid)?
            .0
            .optical_ref
            .lock_opm()?
            .ports()
    };

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
    let new = update.into_inner();
    let document = data.document.lock();

    // `GET /ports` (and hence the GUI) reports an inverted node's ports through the inversion-aware
    // `OpticNode::ports()`, which swaps Input/Output. The `port_type` sent back here is therefore in
    // that *logical* space, but `raw_ports()` stores the *physical* ports (its own `inverted` flag is
    // never set - see `OpticNode::ports`/`NodeAttr::set_inverted`). Translate the logical direction to
    // the physical one via `NodeAttr::inverted()` and store the physical `port_type` in the command, so
    // the lookup here and every later undo/redo resolve against the correct raw port map.
    let (physical_type, old) = document.scenery().with_node_attr(uuid, |node_attr| {
        let physical_type = if node_attr.inverted() {
            port_type.opposite()
        } else {
            port_type
        };
        let port_map = node_attr.raw_ports().ports(&physical_type);
        port_map.get(&port_name).map_or_else(
            || {
                Err(OpossumError::Other(format!(
                    "{physical_type} port '{port_name}' not found"
                )))
            },
            |port| {
                Ok((
                    physical_type,
                    UpdatePortRequest {
                        aperture: new.aperture.is_some().then(|| port.aperture.clone()),
                        coating: new.coating.is_some().then_some(port.coating),
                        lidt: new.lidt.is_some().then_some(port.lidt),
                    },
                ))
            },
        )
    })??;
    let parent_group_id = parent_group_id_or_self(document.scenery(), uuid)?;

    let command = Command::PatchPort(PatchPort {
        uuid,
        parent_group_id,
        port_type: physical_type,
        port_name,
        old,
        new,
    });
    apply_and_push_undo(&data, document, command, true)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::document::undo_document;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{
        coatings::CoatingType,
        nodes::Dummy,
        types::api_types::{DocumentChange, NodeEditorPanel, UndoRedoResponse},
    };

    fn create_test_state() -> Data<AppState> {
        Data::new(AppState::default())
    }

    /// Regression test: `DocumentChange::NodeDetailsChanged` for a port patch must carry the node's
    /// `graph_id` and tag `panel: NodeEditorPanel::PortConfig`, so the GUI's
    /// auto-select-and-open-panel feature can locate and reveal the right node/panel on undo/redo.
    #[actix_web::test]
    async fn test_patch_port_reports_graph_id_and_port_config_panel() {
        let app_state = create_test_state();
        let (root_id, node_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let node_id = document.scenery_mut().add_node(Dummy::default()).unwrap();
            (root_id, node_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(patch_port)
                .service(undo_document),
        )
        .await;

        let update_req = UpdatePortRequest {
            aperture: None,
            coating: Some(CoatingType::default()),
            lidt: None,
        };
        let req = test::TestRequest::patch()
            .uri(&format!("/{node_id}/ports/Input/input_1"))
            .set_json(&update_req)
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(
            matches!(
                &body.changes[0],
                DocumentChange::NodeDetailsChanged { graph_id, .. } if *graph_id == root_id
            ),
            "a port patch must report a details refresh on the node's graph_id"
        );
        assert_eq!(
            body.jump.expect("an undo must carry a jump target").panel,
            Some(NodeEditorPanel::PortConfig),
            "a port patch must jump to the PortConfig panel"
        );
    }

    /// Regression test for the bug where editing the port config of an *inverted* node silently
    /// failed: `GET /ports` reports an inverted node's ports through the inversion-aware
    /// `OpticNode::ports()` (Input/Output swapped), so the GUI round-trips the swapped `port_type`
    /// back here - but `patch_port` resolved it against `raw_ports()`, whose own `inverted` flag is
    /// never set, so the swapped name wasn't in the expected physical map and the PATCH 400'd. No
    /// undo entry was pushed and the edit was lost. `patch_port` now translates the logical
    /// direction to the physical one via `NodeAttr::inverted()`. Inverts a `Dummy` (so its physical
    /// `output_1` shows up under *inputs*), patches `Input/output_1`, and asserts the edit lands on
    /// the physical output port and is undoable to the PortConfig panel.
    #[actix_web::test]
    async fn test_patch_port_on_inverted_node_targets_physical_port() {
        use opossum_core::core_optics::PortType;

        let app_state = create_test_state();
        let node_id = {
            let mut document = app_state.document.lock();
            let node_id = document.scenery_mut().add_node(Dummy::default()).unwrap();
            // Invert the node: its physical `output_1` is now shown (and edited) as a logical input.
            document
                .scenery_mut()
                .with_node_attr_mut(node_id, |a| a.set_inverted(true))
                .unwrap();
            node_id
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(patch_port)
                .service(undo_document),
        )
        .await;

        // The GUI edits the physical `output_1` under the (inverted) Input direction.
        let update_req = UpdatePortRequest {
            aperture: None,
            coating: Some(CoatingType::Fresnel),
            lidt: None,
        };
        let req = test::TestRequest::patch()
            .uri(&format!("/{node_id}/ports/Input/output_1"))
            .set_json(&update_req)
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT,
            "patching an inverted node's port must succeed, not 400 on a raw-port lookup miss"
        );

        // The change must land on the *physical* output port, not be lost.
        let physical_coating = |state: &Data<AppState>| {
            state
                .document
                .lock()
                .scenery()
                .with_node_attr(node_id, |a| {
                    a.raw_ports()
                        .ports_raw(&PortType::Output)
                        .get("output_1")
                        .map(|p| p.coating)
                })
                .unwrap()
        };
        assert_eq!(
            physical_coating(&app_state),
            Some(CoatingType::Fresnel),
            "the coating edit must apply to the physical output port"
        );

        // One undo reverts it and jumps to the Port Config panel (the symptom the user reported).
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert_eq!(
            body.jump.expect("an undo must carry a jump target").panel,
            Some(NodeEditorPanel::PortConfig),
            "undoing an inverted node's port edit must jump to the PortConfig panel"
        );
        assert_eq!(
            physical_coating(&app_state),
            Some(CoatingType::default()),
            "undo must restore the physical output port's original coating"
        );
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
            response.outputs.contains_key("ext_out_1"),
            "the group's mapped output port must be reported, not an empty list; got {:?}",
            response.outputs.keys().collect::<Vec<_>>()
        );
        assert!(
            response.inputs.is_empty(),
            "the group has no mapped input port"
        );
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
