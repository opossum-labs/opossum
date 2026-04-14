use crate::{app_state::AppState, error::BackEndErrorResponse};
use actix_web::{
    HttpResponse, Responder, get,
    guard::GuardContext,
    http::header,
    post,
    web::{self, Json},
};
use opossum_core::{
    core_optics::NodeAttr,
    error::OpossumError,
    prelude::{OpmDocument, Proptype},
    utils::LockExt,
};
use parking_lot::MutexGuard;
use uuid::Uuid;

/// Update a property of an optical node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "Update a single property of the optical node"),
    ),
    request_body(content = String,
        description = "updated property of node",
        content_type = "application/ron",
        example= "(\"key\", \"value\")"
    ),
    responses(
        (status = OK, description = "Node property successfully updated"),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/ron")
    )
)]
#[post("/property/{uuid}")]
async fn post_node_property(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let (prop_key, prop_value): (String, Proptype) = match ron::de::from_str(body.as_str()) {
        Ok((key, proptype)) => (key, proptype),
        Err(e) => {
            return Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                &format!("Failed to deserialize property value: {e}"),
            ));
        }
    };
    let mut document = data.document.lock();
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            match node_attr.set_property(prop_key.as_str(), prop_value) {
                Ok(()) => Ok(HttpResponse::Ok()
                    .content_type("application/ron")
                    .body(ron::ser::to_string("").unwrap())),
                Err(e) => Err(BackEndErrorResponse::new(
                    400,
                    "Opossum",
                    e.to_string().as_str(),
                )),
            }
        })
        .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))?
}
/// Get all properties of the specified node in either JSON or RON format.
///
/// Return all properties (`NodeAttr`) of the node specified by its UUID.
/// The format is determined by the `Accept` header.
/// Defaults to `application/json` if the header is missing or doesn't specify
/// `application/ron`.
///
/// # Important
///
/// Due to the fact that numeric properties can have values such as `nan` or `inf` it is possible to read
/// the data as RON. The standard JSON format does **not** support encoding of these values. They are simply
/// returned as `null` values.
///
/// - **Note**: This function only returns `NodeAttr`, even for group nodes.
///   A possible `graph` structure is omitted.
/// - **Note**: This function searches the node recursively in the whole scenery.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    responses(
        (status = OK, description = "get all node properties", content(("application/json"),("application/ron"))),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/properties", guard = "wants_ron_guard")]
async fn get_properties_ron(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock();
    let (node_attr, is_reference) = get_referenced_node_attr_from_state(false, uuid, &document)?;
    drop(document);
    let body = ron::ser::to_string_pretty(
        &(node_attr, is_reference),
        ron::ser::PrettyConfig::new().new_line("\n"),
    )
    .map_err(|e| OpossumError::Other(format!("RON Serialization Error: {e}")))?;

    Ok(HttpResponse::Ok()
        .content_type("application/ron")
        .body(body))
}
/// helper function for checking the ACCEPT header.
fn wants_ron_guard(ctx: &GuardContext<'_>) -> bool {
    if let Some(val) = ctx.head().headers.get(header::ACCEPT)
        && let Ok(s) = val.to_str()
    {
        return s.contains("application/ron");
    }
    false
}
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    responses(
        (status = OK, description = "get all node properties", content(("application/json"),("application/ron"))),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/properties")]
async fn get_properties_json(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<NodeAttr>, BackEndErrorResponse> {
    let node_attr = get_node_attr_from_state(path.into_inner(), &data)?;
    Ok(Json(node_attr))
}
// Helper function to contain the core logic
/// Retrieve the node attributes of a node, referenced by a reference-node
/// To signal the GUI, that the `node_attributes` are readonly when it is a reference, a bool will be sent if it is a reference or not
/// true: node is a reference
/// false: node is original
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
                "'reference id' property not found",
            ))
        }
    } else {
        Ok((node_attr, is_reference))
    }
}
// Helper function to contain the core logic
fn get_node_attr_from_state(
    uuid: Uuid,
    data: &web::Data<AppState>,
) -> Result<NodeAttr, BackEndErrorResponse> {
    let document = data.document.lock();
    let node_attr = document
        .scenery()
        .node_recursive(uuid)?
        .0
        .optical_ref
        .lock_opm()?
        .node_attr()
        .clone();
    Ok(node_attr)
}
