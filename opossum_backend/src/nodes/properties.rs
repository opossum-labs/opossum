use crate::{app_state::AppState, error::BackEndErrorResponse};
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
pub async fn get_properties(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req: HttpRequest, // Wir lesen den Header direkt hier aus!
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock();

    // Löst die Node auf und prüft, ob es eine Referenz ist
    let (node_attr, is_reference) = get_referenced_node_attr_from_state(false, uuid, &document)?;

    let response_data = NodePropertiesResponse {
        properties: node_attr.properties().clone(),
        is_reference,
    };

    // Content Negotiation (ersetzt den alten wants_ron_guard)
    let wants_ron = req
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|h| h.to_str().ok())
        .map_or(false, |s| s.contains("application/ron"));

    if wants_ron {
        let body = ron::ser::to_string_pretty(
            &response_data,
            ron::ser::PrettyConfig::new().new_line("\n"),
        )
        .map_err(|e| BackEndErrorResponse::new(500, "Serialization Error", &e.to_string()))?;
        Ok(HttpResponse::Ok()
            .content_type("application/ron")
            .body(body))
    } else {
        Ok(HttpResponse::Ok().json(response_data))
    }
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
        (status = OK, description = "Property successfully updated"),
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

    // 1. Deserialisiere den gesendeten RON-String sicher in ein `Proptype` Enum
    let new_value: Proptype = ron::from_str(&body).map_err(|e| {
        BackEndErrorResponse::new(
            400,
            "Parse Error",
            &format!(
                "Failed to parse RON value for property '{}': {}",
                prop_name, e
            ),
        )
    })?;

    // 2. Wende den neuen Wert auf das Modell an
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            node_attr.set_property(&prop_name, new_value)
        })??; // Double unwrap: Erstes Result vom Closure, zweites Result von with_node_attr_mut

    Ok(HttpResponse::Ok().finish())
}

// --- Helper Functions ---

/// Retrieve the node attributes of a node, resolving references if necessary.
/// Returns a tuple of the `NodeAttr` and a boolean indicating if it was a reference.
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
            // Rekursiver Aufruf, um die echte Node zu finden
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
