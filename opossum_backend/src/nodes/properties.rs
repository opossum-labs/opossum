use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{parent_group_id_or_self, ron_or_json_response},
    undo::{Command, PatchProperty},
};
use actix_web::{HttpRequest, HttpResponse, get, patch, web};
use opossum_core::{
    core_optics::NodeAttr,
    prelude::{OpmDocument, Proptype},
    types::api_types::{ErrorResponse, NodePropertiesResponse},
    utils::LockExt,
};
use parking_lot::MutexGuard;
use uuid::Uuid;

/// Get all custom properties of an optical node
///
/// Returns the properties map of the node specified by its UUID.
/// Supports Content Negotiation: Use `Accept: application/ron` for RON format (required for `NaN`/`Inf`),
/// otherwise defaults to `application/json`.
#[utoipa::path(
    tag = "node",
    params(("uuid" = Uuid, Path, description = "UUID of the optical node")),
    responses(
        (status = OK, description = "Get custom properties map", content(
            (NodePropertiesResponse = "application/json"),
            (NodePropertiesResponse = "application/ron")
        )),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/properties")]
#[allow(clippy::future_not_send)]
pub async fn get_properties(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock();

    let (node_attr, is_reference) = get_referenced_node_attr_from_state(false, uuid, &document)?;

    let response_data = NodePropertiesResponse {
        properties: node_attr.properties().clone(),
        is_reference,
    };

    ron_or_json_response(&req, &response_data)
}

/// Update a specific property of an optical node
///
/// This endpoint updates exactly one property. Since numeric values can contain `NaN` or `Infinity`,
/// the request body MUST be formatted as a RON string representing the `Proptype` enum
/// (e.g., `Length(1.5)` or `Bool(true)`).
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
        ("prop_name" = String, Path, description = "Name of the property to update (e.g., 'focal length')")
    ),
    request_body(
        content = String,
        description = "The new property value as a RON string (e.g., `Length(0.05)`)",
        content_type = "application/ron"
    ),
    responses(
        (status = NO_CONTENT, description = "Property successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID/Property not found or invalid RON format", content_type="application/json")
    )
)]
#[patch("/{uuid}/properties/{prop_name}")]
pub async fn patch_property(
    data: web::Data<AppState>,
    path: web::Path<(Uuid, String)>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let (uuid, prop_name) = path.into_inner();

    let new_value: Proptype = ron::from_str(&body).map_err(|e| {
        BackEndErrorResponse::new(
            400,
            "Parse Error",
            &format!("Failed to parse RON value for property '{prop_name}': {e}"),
        )
    })?;

    let mut document = data.document.lock();
    let old_value = document.scenery().with_node_attr(uuid, |node_attr| {
        node_attr.properties().get(&prop_name).cloned()
    })??;
    let parent_group_id = parent_group_id_or_self(document.scenery(), uuid)?;

    let inverse = Command::PatchProperty(PatchProperty {
        uuid,
        parent_group_id,
        prop_name,
        old: old_value,
        new: new_value,
    })
    .apply(&mut document)?;
    data.push_undo(inverse);
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

// --- Helper Functions ---

fn get_referenced_node_attr_from_state(
    mut is_reference: bool,
    uuid: Uuid,
    document: &MutexGuard<'_, OpmDocument>,
) -> Result<(NodeAttr, bool), BackEndErrorResponse> {
    let node_attr = document
        .scenery()
        .node_recursive(uuid)?
        .0
        .optical_ref
        .lock_opm()?
        .node_attr()
        .clone();

    if node_attr.node_type() == "reference" {
        is_reference = true;
        let ref_node_props = node_attr.properties();
        if let Ok(Proptype::Uuid(ref_uuid)) = ref_node_props.get("reference id") {
            get_referenced_node_attr_from_state(is_reference, *ref_uuid, document)
        } else {
            Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                "'reference id' property not found on reference node",
            ))
        }
    } else {
        Ok((node_attr, is_reference))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::document::undo_document;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{
        core_optics::node_attr::HasNodeAttr,
        nodes::Dummy,
        types::api_types::{DocumentChange, NodeEditorPanel, UndoRedoResponse},
    };

    fn create_test_state() -> Data<AppState> {
        Data::new(AppState::default())
    }

    /// Regression test: `DocumentChange::NodeDetailsChanged` for a property patch must carry the
    /// node's `graph_id` and tag `panel: NodeEditorPanel::Properties`, so the GUI's
    /// auto-select-and-open-panel feature can locate and reveal the right node/panel on undo/redo.
    #[actix_web::test]
    async fn test_patch_property_reports_graph_id_and_properties_panel() {
        let app_state = create_test_state();
        let (root_id, node_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let node_id = document.scenery_mut().add_node(Dummy::default()).unwrap();
            document
                .scenery_mut()
                .with_node_attr_mut(node_id, |attr| {
                    attr.create_property("test_prop", "test", Proptype::Bool(false))
                })
                .unwrap()
                .unwrap();
            (root_id, node_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(patch_property)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::patch()
            .uri(&format!("/{node_id}/properties/test_prop"))
            .set_payload("Bool(true)")
            .insert_header(("Content-Type", "application/ron"))
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
            "a property patch must report a details refresh on the node's graph_id"
        );
        assert_eq!(
            body.jump.expect("an undo must carry a jump target").panel,
            Some(NodeEditorPanel::Properties),
            "a property patch must jump to the Properties panel"
        );
    }

    #[actix_web::test]
    async fn test_get_properties_invalid_uuid() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(get_properties)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/{}/properties", Uuid::new_v4()))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_patch_property_invalid_ron() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(patch_property)).await;

        let req = test::TestRequest::patch()
            .uri(&format!("/{}/properties/focal_length", Uuid::new_v4()))
            .set_payload("INVALID_RON")
            .insert_header(("Content-Type", "application/ron"))
            .to_request();

        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let error_body: ErrorResponse = test::read_body_json(resp).await;
        assert_eq!(error_body.category, "Parse Error");
    }
}
