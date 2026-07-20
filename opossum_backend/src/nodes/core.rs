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

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::capture_node_connections,
    undo::{AddNode, Command, PatchNode, RemoveNode, capture_old_node_request},
};

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

    data.push_undo(Command::RemoveNode(RemoveNode {
        parent_group_id: uuid,
        node: new_node_ref.clone(),
        cascaded: Vec::new(),
        connections: Vec::new(),
    }));

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
    let new = update.into_inner();
    let mut document = data.document.lock();

    let old = document
        .scenery()
        .with_node_attr(uuid, |node_attr| capture_old_node_request(node_attr, &new))?;
    let parent_group_id = document.scenery().node_recursive(uuid)?.1;

    let inverse = Command::PatchNode(PatchNode {
        uuid,
        parent_group_id,
        old,
        new,
    })
    .apply(&mut document)?;
    data.push_undo(inverse);
    drop(document);

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

    // Capture the target node and, since deleting it cascades to any reference nodes pointing at it
    // (see `NodeGroup::delete_node`), every one of those too - each as a live `OpticRef` handle plus
    // its own parent group, so undo can restore the whole cascade exactly as it was.
    let (target_ref, parent_group_id, cascaded) = {
        let scenery = document.scenery();
        let (target_ref, parent_group_id) = scenery.node_recursive(uuid)?;
        let referring = scenery
            .graph()
            .find_all_nodes_referring_to_uuid(uuid, scenery.node_attr().uuid())?;
        let mut cascaded = Vec::new();
        for ref_ids in referring.values() {
            for ref_id in ref_ids {
                if let Ok((r, p)) = scenery.node_recursive(*ref_id) {
                    cascaded.push((p, r));
                }
            }
        }
        (target_ref, parent_group_id, cascaded)
    };

    // Captured before deletion, since `delete_node` silently drops the node's incident edges in its
    // parent graph - without this, undo would restore the node but leave it disconnected (bug 4).
    let connections =
        capture_node_connections(document.scenery(), parent_group_id, uuid).unwrap_or_default();

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

    // Only claim cascaded nodes that `delete_node` actually removed, in case its cascade rules ever
    // diverge from what `find_all_nodes_referring_to_uuid` predicted.
    let cascaded: Vec<(Uuid, OpticRef)> = cascaded
        .into_iter()
        .filter(|(_, r)| r.uuid().is_ok_and(|id| deleted_nodes.contains(&id)))
        .collect();
    data.push_undo(Command::AddNode(AddNode {
        parent_group_id,
        node: target_ref,
        cascaded,
        connections,
    }));

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

#[cfg(test)]
mod test {
    use super::*;
    use crate::document::undo_document;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{millimeter, nodes::Dummy, utils::geom_transformation::Isometry};

    /// Regression test for the bug where undoing an alignment change didn't restore the old value.
    /// `UpdateNodeRequest::alignment` used to be a single `Option`, which can express "set to X" but
    /// not "clear back to unset" - so capturing the old value as `None` (the node's alignment was
    /// unset before the edit) silently did nothing on undo. Covers both the previously-broken
    /// unset-to-set case and the already-working set-to-different-set case.
    #[actix_web::test]
    async fn test_undo_alignment_change_restores_old_value() {
        let app_state = Data::new(AppState::default());
        let node_id = {
            let mut document = app_state.document.lock();
            document.scenery_mut().add_node(Dummy::default()).unwrap()
        };
        // Confirm the node starts with no alignment set - the case that was silently broken.
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .with_node_attr(node_id, |attr| attr.alignment().is_none())
                .unwrap()
        );

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(patch_node)
                .service(undo_document),
        )
        .await;

        let iso_a = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        let req = test::TestRequest::patch()
            .uri(&format!("/{node_id}"))
            .set_json(&UpdateNodeRequest {
                alignment: Some(Some(iso_a)),
                ..Default::default()
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        let iso_b = Isometry::new_along_z(millimeter!(20.0)).unwrap();
        let req = test::TestRequest::patch()
            .uri(&format!("/{node_id}"))
            .set_json(&UpdateNodeRequest {
                alignment: Some(Some(iso_b)),
                ..Default::default()
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // First undo: alignment must go from iso_b back to iso_a (the already-working case).
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            app_state
                .document
                .lock()
                .scenery()
                .with_node_attr(node_id, |attr| *attr.alignment())
                .unwrap(),
            Some(iso_a),
            "undo must restore the previous concrete alignment value"
        );

        // Second undo: alignment must go from iso_a back to unset (the case that was broken).
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            app_state
                .document
                .lock()
                .scenery()
                .with_node_attr(node_id, |attr| *attr.alignment())
                .unwrap(),
            None,
            "undo must clear the alignment back to unset, not leave it at iso_a"
        );
    }

    /// Regression test for the bug where undoing the deletion of a *connected* node only restored the
    /// node itself, not its connections in the parent graph - `delete_node` never captured them before
    /// calling `scenery.delete_node`, unlike the copy/paste flow's `capture_node_connections` use (see
    /// `helper_functions.rs`). Not group-specific - any deleted node with parent-graph connections lost
    /// them on undo - but most visible for groups, which typically have more external wiring, so this
    /// mirrors `test_undo_group_conversion_restores_internal_and_boundary_connections` in
    /// `document.rs`: converts `{node_a, node_b}` into a group connected to `node_c`, deletes the group
    /// node, undoes the deletion, and asserts both the group and its external connection to `node_c` are
    /// restored.
    #[actix_web::test]
    async fn test_undo_delete_group_node_restores_external_connection() {
        use crate::document::undo_document;
        use opossum_core::{meter, nodes::NodeGroup, types::api_types::ConvertToGroupRequest};

        let app_state = Data::new(AppState::default());
        let (root_id, node_a, node_b, node_c) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();
            let node_a = scenery.add_node(Dummy::default()).unwrap();
            let node_b = scenery.add_node(Dummy::default()).unwrap();
            let node_c = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(node_a, "output_1", node_b, "input_1", meter!(0.1))
                .unwrap();
            scenery
                .connect_nodes(node_b, "output_1", node_c, "input_1", meter!(0.2))
                .unwrap();
            (root_id, node_a, node_b, node_c)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(crate::operations::post_convert_nodes_to_group)
                .service(delete_node)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/convert_to_group")
            .set_json(&ConvertToGroupRequest {
                group_id: root_id,
                nodes_to_convert: vec![node_a, node_b],
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let group_id = app_state
            .document
            .lock()
            .scenery()
            .node_recursive(node_a)
            .unwrap()
            .1;

        let req = test::TestRequest::delete()
            .uri(&format!("/{group_id}"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(group_id)
                .is_err(),
            "group node must be gone after delete"
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "undo of the delete must not error"
        );

        let document = app_state.document.lock();
        assert!(
            document.scenery().node_recursive(group_id).is_ok(),
            "group node must be restored after undo"
        );
        assert!(document.scenery().node_recursive(node_c).is_ok());

        let connections = document
            .scenery()
            .with_group_node(root_id, NodeGroup::connections)
            .unwrap();
        assert!(
            connections
                .iter()
                .any(|c| c.src_id == group_id && c.target_id == node_c),
            "the group node's external connection to node_c must be restored"
        );
    }
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
