use std::collections::HashMap;

use actix_web::{
    HttpRequest, HttpResponse, delete, get, patch, post,
    web::{self, Json},
};
use nalgebra::Point2;
use opossum_core::{
    core_optics::{OpticRef, node_attr::HasNodeAttr},
    error::OpossumError,
    light::lightdata::{energy_data_builder::EnergyDataBuilder, ray_data_builder::RayDataBuilder},
    nodes::{NodeReference, create_node_ref},
    prelude::{AnalyzerType, OpmDocument, Proptype},
    types::api_types::{ErrorResponse, NewNode, NewRefNode, NodeInfo, UpdateNodeRequest},
    utils::LockExt,
};
use parking_lot::MutexGuard;
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

/// Get all nodes of a group node
///
/// Return a list of all nodes of a group node specified by its UUID.
/// - **Note**: This function searches recursively for the UUID in the whole scenery.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "get all nodes of the group", content((Vec<NodeInfo> = "application/json"))),
        (status = BAD_REQUEST, description = "UUID not found or not a group node", content((ErrorResponse = "application/json")))
    )
)]
#[get("/{uuid}/children")]
async fn get_children(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<NodeInfo>>, BackEndErrorResponse> {
    let document = data.document.lock();
    let scenery = document.scenery().clone();
    drop(document);
    let uuid = path.into_inner();

    let nodes_info = scenery.with_group_node(uuid, |g| {
        g.nodes()
            .iter()
            .map(|n| {
                let node = n.optical_ref.lock_opm()?; // <- Kein unwrap() mehr!
                let node_info = NodeInfo::from_analyzable(&*node, None);
                drop(node);
                Ok(node_info)
            })
            .collect::<Result<Vec<NodeInfo>, OpossumError>>()
    })??;
    Ok(Json(nodes_info))
}

/// Add a new node to a group node
///
/// This function adds a new optical node to a group node specified by its UUID.
/// - The node type as well as the coordinates of the corresponding GUI element must be given.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    request_body(content = NewNode,
        description = "type and GUI position of node the optical node to be created",
        content_type = "application/json",
        example ="{\"node_type\": \"dummy\", \"gui_position\": [0.0,0.0]}"
    ),
    responses(
        (status = CREATED, body= NodeInfo, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Node of the given type not found, UUID not found, no group node", content_type="application/json")
    )
)]
#[post("/{uuid}/children")]
async fn post_children(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    node_type: web::Json<NewNode>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let new_node_info = node_type.into_inner();
    let new_node_ref = create_node_ref(new_node_info.node_type())?;
    let mut node = new_node_ref.optical_ref.lock_opm()?;
    let node_attr = node.node_attr_mut();
    node_attr.set_gui_position(Some(Point2::new(
        new_node_info.gui_position().0,
        new_node_info.gui_position().1,
    )));
    drop(node);

    let mut document = data.document.lock();
    let uuid = path.into_inner();
    let scenery = document.scenery_mut();

    let _ = scenery.with_group_node_mut(uuid, |g| g.add_node_ref(new_node_ref.clone()))??;

    // --- AUTOMATICALLY INJECT MAPPINGS INTO ALL ANALYZERS IF NEW NODE IS A SOURCE PORT ---
    let node_type_str = new_node_ref
        .optical_ref
        .lock_opm()?
        .node_attr()
        .node_type()
        .to_string();
    let new_node_uuid = new_node_ref.optical_ref.lock_opm()?.node_attr().uuid();

    if node_type_str == "source port" {
        let analyzer_keys: Vec<Uuid> = document.analyzers().keys().copied().collect();
        for az_uuid in analyzer_keys {
            if let Some(analyzer_info) = document.analyzer_mut(az_uuid) {
                let mut a_type = analyzer_info.analyzer_type().clone();
                match &mut a_type {
                    AnalyzerType::Energy(cfg) => {
                        cfg.map_source(new_node_uuid, EnergyDataBuilder::default());
                    }
                    AnalyzerType::RayTrace(cfg) => {
                        cfg.map_source(new_node_uuid, RayDataBuilder::default());
                    }
                    AnalyzerType::GhostFocus(cfg) => {
                        cfg.map_source(new_node_uuid, RayDataBuilder::default());
                    }
                }
                analyzer_info.set_analyzer_type(&a_type);
            }
        }
    }

    drop(document);

    let node = new_node_ref.optical_ref.lock_opm()?;
    let node_info = NodeInfo::from_analyzable(&*node, None);
    drop(node);
    Ok(HttpResponse::Created().json(node_info))
}

/// Get optical node properties
///
/// This function retrieves the properties of an optical node specified by its UUID. It also searches for the node recursively in the whole scenery.
/// Supports Content Negotiation: Use `Accept: application/ron` for RON format,
/// otherwise defaults to `application/json`.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    responses(
        (status = OK, description = "get all node properties", content((NodeInfo = "application/json"),(NodeInfo ="application/ron"))),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}")]
#[allow(clippy::future_not_send)]
async fn get_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock();
    // Retrieve the node info
    let node_ref = document.scenery().node_recursive(uuid)?.0;
    let node = node_ref.optical_ref.lock_opm()?;
    let node_info = NodeInfo::from_analyzable(&*node, None);
    drop(node);
    drop(document);
    // Content Negotiation
    let wants_ron = req
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|s| s.contains("application/ron"));

    if wants_ron {
        // Serialize to RON using pretty formatting
        let body =
            ron::ser::to_string_pretty(&node_info, ron::ser::PrettyConfig::new().new_line("\n"))
                .map_err(|e| {
                    BackEndErrorResponse::new(500, "Serialization Error", &e.to_string())
                })?;

        Ok(HttpResponse::Ok()
            .content_type("application/ron")
            .body(body))
    } else {
        // Fallback to JSON
        Ok(HttpResponse::Ok().json(node_info))
    }
}

/// Update optical node properties
///
/// Modifies the standard properties (name, inversion, isometries, GUI position) of an optical node
/// specified by its UUID. Only the fields provided in the request body will be updated.
#[utoipa::path(
    tag = "node",
    request_body = UpdateNodeRequest,
    responses(
        (status = NO_CONTENT, description = "Node properties successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found or invalid data")
    )
)]
#[patch("/{uuid}")]
async fn patch_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    update: web::Json<UpdateNodeRequest>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let update = update.into_inner();
    data.document
        .lock()
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            if let Some(name) = update.name {
                node_attr.set_name(&name);
            }
            if let Some(inverted) = update.inverted {
                node_attr.set_inverted(inverted);
            }
            if let Some(iso_opt) = update.isometry {
                node_attr.set_isometry_option(iso_opt);
            }
            if let Some(align) = update.alignment {
                node_attr.set_alignment(align);
            }
            if let Some(gui_pos_opt) = update.gui_position {
                node_attr.set_gui_position(gui_pos_opt.map(|(x, y)| Point2::new(x, y)));
            }
            Ok::<(), OpossumError>(())
        })??;

    Ok(HttpResponse::NoContent().finish())
}

/// Delete a node
///
/// This function deletes a node. It also deletes reference nodes which refer to this node.
/// A list of UUIDs of the effectively deleted nodes is returned.
#[utoipa::path(tag = "node",
responses(
    (status = OK, body= Vec<Uuid>, description = "UUIDs of the deleted nodes", content_type="application/json"),
    (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
))]
#[delete("/{uuid}")]
async fn delete_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock();
    let scenery = document.scenery_mut();
    let deleted_nodes = scenery.delete_node(uuid)?;

    // --- AUTOMATICALLY REMOVE OBSOLETE MAPPINGS FROM ALL ANALYZERS ---
    for deleted_uuid in &deleted_nodes {
        let analyzer_keys: Vec<Uuid> = document.analyzers().keys().copied().collect();
        for az_uuid in analyzer_keys {
            if let Some(analyzer_info) = document.analyzer_mut(az_uuid) {
                let mut a_type = analyzer_info.analyzer_type().clone();
                match &mut a_type {
                    AnalyzerType::Energy(cfg) => {
                        let _ = cfg.remove_source(deleted_uuid);
                    }
                    AnalyzerType::RayTrace(cfg) => {
                        let _ = cfg.remove_source(deleted_uuid);
                    }
                    AnalyzerType::GhostFocus(cfg) => {
                        let _ = cfg.remove_source(deleted_uuid);
                    }
                }
                analyzer_info.set_analyzer_type(&a_type);
            }
        }
    }

    drop(document);
    Ok(web::Json(deleted_nodes))
}

/// Get nodes that reference a certain node uuid
///
/// A list of UUIDs of the nodes that reference the passed uuid is returned.
#[utoipa::path(tag = "node",
responses(
    (status = OK, body= HashMap<Uuid, Vec<Uuid>>, description = "UUIDs of the reference nodes, sorted by the group in which they are contained", content_type="application/json"),
    (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
))]
#[get("/{uuid}/references")]
async fn get_reference_nodes(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<HashMap<Uuid, Vec<Uuid>>>, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock();
    let scenery = document.scenery();
    let references = scenery
        .graph()
        .find_all_nodes_referring_to_uuid(uuid, scenery.node_attr().uuid())?;
    drop(document);
    Ok(web::Json(references))
}

/// Add a new reference node to a group node
///
/// Adds a new reference node to the specified group node, identified by its UUID (provided in the path).
/// The reference node will refer to another node, specified by its UUID in the request body.
///
/// - The UUID of the node to be referenced, as well as the coordinates of the corresponding GUI element, must be provided.
/// - The function returns information about the newly created reference node.
///
/// # Parameters
/// - `uuid`: UUID of the group node to which the reference node will be added (provided in the path).
/// - `referring_node`: UUID of the node to be referenced (provided in the request body).
///
/// # Returns
/// - On success: Information about the newly created reference node.
/// - On error: An error response if the UUID is not found or the target is not a group
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    request_body(content = NewRefNode,
        description = "UUID of the node to be referred to and GUI position of the optical node to be created",
        content_type = "application/json",
        example ="{\"referring_node\": \"3fa85f64-5717-4562-b3fc-2c963f66afa6\", \"gui_position\": [0.0,0.0]}"
    ),
    responses(
        (status = CREATED, body= NodeInfo, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found, no group node", content_type="application/json")
    )
)]
#[post("/{uuid}/references")]
async fn post_reference(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    ref_node_info: web::Json<NewRefNode>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let ref_node_info = ref_node_info.into_inner();

    let mut document = data.document.lock();
    let referring_node =
        get_nested_referenced_node_from_state(ref_node_info.referring_node(), &document)?;
    let mut node_reference = NodeReference::from_node(&referring_node)?;

    node_reference
        .node_attr_mut()
        .set_gui_position(Some(Point2::new(
            ref_node_info.gui_position().0,
            ref_node_info.gui_position().1,
        )));

    let scenery = document.scenery_mut();
    let _ = scenery.with_group_node_mut(group_uuid, |g| g.add_node(node_reference.clone()))??;

    drop(document);
    let node_info = NodeInfo::from_analyzable(&node_reference, None);
    Ok(HttpResponse::Created().json(node_info))
}

#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the node"),
    ),
    responses(
        (status = OK, description = "get the group hierarchy of a node", content(("application/json"))),
        (status = BAD_REQUEST, body = ErrorResponse, description = "node with UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/hierarchy")]
async fn get_node_hierarchy(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<(Uuid, String)>>, BackEndErrorResponse> {
    let node_id = path.into_inner();
    let document = data.document.lock();
    let scenery = document.scenery();
    let mut group_hierarchy = scenery.get_node_hierarchy_bottom_up(node_id)?;
    drop(document);
    group_hierarchy.reverse();

    Ok(Json(group_hierarchy))
}

fn get_nested_referenced_node_from_state(
    uuid: Uuid,
    document: &MutexGuard<'_, OpmDocument>,
) -> Result<OpticRef, BackEndErrorResponse> {
    let optic_ref = document.scenery().node_recursive(uuid)?.0;
    let node_attr = optic_ref.optical_ref.lock_opm()?.node_attr().clone();
    if node_attr.node_type() == "reference" {
        let ref_node_props = node_attr.properties();
        if let Ok(Proptype::Uuid(ref_uuid)) = ref_node_props.get("reference id") {
            get_nested_referenced_node_from_state(*ref_uuid, document)
        } else {
            Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                "'reference id' property not found",
            ))
        }
    } else {
        Ok(optic_ref)
    }
}
