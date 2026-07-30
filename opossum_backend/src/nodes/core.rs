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
    types::api_types::{
        DeleteNodeResponse, ErrorResponse, NewNode, NewRefNode, NodeInfo, UpdateNodeRequest,
    },
    utils::LockExt,
};
use parking_lot::MutexGuard;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{
        capture_node_connections, disconnect_exposed_port_cascades_for_node,
        parent_group_id_or_self, split_cascades_for_response,
    },
    undo::{Command, NodeSnapshot, PatchAnalyzer, PatchNode, capture_old_node_request},
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

    // Auto-injecting a source-port mapping mutates each analyzer's config as a side effect - capture the
    // inverse (restore the analyzer's pre-injection config) per changed analyzer, so undoing this add also
    // strips the mappings it injected instead of leaving them dangling on a removed node.
    let mut analyzer_inverses: Vec<Command> = Vec::new();
    if node_type_str == "source port" {
        let analyzer_keys: Vec<Uuid> = document.analyzers().keys().copied().collect();
        for az_uuid in analyzer_keys {
            if let Some(analyzer_info) = document.analyzer_mut(az_uuid) {
                let old_type = analyzer_info.analyzer_type().clone();
                let mut a_type = old_type.clone();
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
                if a_type != old_type {
                    analyzer_info.set_analyzer_type(&a_type);
                    analyzer_inverses.push(Command::PatchAnalyzer(PatchAnalyzer {
                        id: az_uuid,
                        old: a_type,
                        new: old_type,
                    }));
                }
            }
        }
    }

    drop(document);

    let remove_node = Command::RemoveNode(NodeSnapshot {
        parent_group_id: uuid,
        node: new_node_ref.clone(),
        cascaded: Vec::new(),
        connections: Vec::new(),
    });
    if analyzer_inverses.is_empty() {
        data.push_undo(remove_node);
    } else {
        // One add = one undo step: removing the node and restoring every analyzer it touched.
        let mut batch = vec![remove_node];
        batch.extend(analyzer_inverses);
        data.push_undo(Command::Batch(batch));
    }

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
/// specified by its UUID. Only the fields provided in the request body will be updated. Patching
/// the scenery root itself is applied but not recorded as an undo step - the root is only ever
/// patched programmatically (see the comment in the handler).
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
    let parent_group_id = parent_group_id_or_self(document.scenery(), uuid)?;
    // `parent_group_id_or_self` reports the scenery root as its own parent, so this equality holds
    // exactly for the root. A root patch is never a user edit - the GUI renames the root to mirror
    // the project's file name on new project/load/save, and there is no UI path for a user to
    // rename it - so recording it would leave a phantom undo step right after opening a new or
    // loaded project (and needlessly wipe the redo stack on save-under-a-new-name).
    let is_root = parent_group_id == uuid;

    let mut commands = vec![Command::PatchNode(PatchNode {
        uuid,
        parent_group_id,
        old,
        new: new.clone(),
    })];

    // A rename propagates to every reference node pointing at `uuid` - they store their own name copy
    // (`ref (name)`, see `NodeReference`), so without this the model (and saved `.opm`) keeps stale
    // reference names. Capturing them as extra `PatchNode`s in the same batch makes the whole rename a
    // single undo step (previously the GUI fanned out one PATCH per reference = one undo step each).
    // Skipped for the root, which has no references.
    if let Some(name) = &new.name
        && !is_root
    {
        let ref_name = format!("ref ({name})");
        let root_uuid = document.scenery().node_attr().uuid();
        let referring = document
            .scenery()
            .graph()
            .find_all_nodes_referring_to_uuid(uuid, root_uuid)?;
        for ref_id in referring.values().flatten() {
            // `find_all_nodes_referring_to_uuid` reports the queried node as its own referrer - skip it.
            if *ref_id == uuid {
                continue;
            }
            let ref_new = UpdateNodeRequest {
                name: Some(ref_name.clone()),
                ..Default::default()
            };
            let ref_old = document.scenery().with_node_attr(*ref_id, |node_attr| {
                capture_old_node_request(node_attr, &ref_new)
            })?;
            let ref_parent = parent_group_id_or_self(document.scenery(), *ref_id)?;
            commands.push(Command::PatchNode(PatchNode {
                uuid: *ref_id,
                parent_group_id: ref_parent,
                old: ref_old,
                new: ref_new,
            }));
        }
    }

    // A single command stays a single command (no needless Batch wrapper); a rename with references
    // becomes one Batch = one undo step.
    let command = if commands.len() == 1 {
        commands.pop().unwrap()
    } else {
        Command::Batch(commands)
    };
    let inverse = command.apply(&mut document)?;
    if !is_root {
        data.push_undo(inverse);
    }
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

/// Delete a node
///
/// This function deletes a node. It also deletes reference nodes which refer to this node.
/// Returns the UUIDs of the effectively deleted nodes, plus any external connections that had to be
/// disconnected as a side effect (e.g. because they depended on a port mapping of the deleted node).
#[utoipa::path(tag = "node",
responses(
    (status = OK, body = DeleteNodeResponse, description = "UUIDs of the deleted nodes and any disconnected connections", content_type="application/json"),
    (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
))]
#[delete("/{uuid}")]
async fn delete_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<DeleteNodeResponse>, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock();
    let (inverse, response) = delete_node_capturing(&mut document, uuid)?;
    drop(document);
    push_delete_inverse(&data, inverse);
    Ok(web::Json(response))
}

/// Delete several nodes in one step
///
/// Deletes every node in the request body (each cascading to its reference nodes and tearing down its
/// exposed-port chains, exactly like [`delete_node`]), pushing a *single* undo entry so one undo
/// restores the whole selection - unlike issuing one `DELETE /{uuid}` per node, which would be one
/// undo step each. A uuid already removed by a prior node's cascade is skipped. Returns the merged
/// [`DeleteNodeResponse`] across all deletions.
#[utoipa::path(tag = "node",
    request_body(content = Vec<Uuid>, description = "UUIDs of the nodes to delete together"),
    responses(
        (status = OK, body = DeleteNodeResponse, description = "Merged deletion result", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "A UUID could not be deleted", content_type="application/json")
    )
)]
#[post("/delete")]
async fn delete_nodes(
    data: web::Data<AppState>,
    body: web::Json<Vec<Uuid>>,
) -> Result<Json<DeleteNodeResponse>, BackEndErrorResponse> {
    let uuids = body.into_inner();
    let mut document = data.document.lock();

    let mut inverse: Vec<Command> = Vec::new();
    let mut merged = DeleteNodeResponse {
        deleted_nodes: Vec::new(),
        disconnected_connections: Vec::new(),
        removed_port_mappings: Vec::new(),
    };
    for uuid in uuids {
        // A node earlier in the selection may have cascade-deleted this one (a reference node); its
        // restoration is already captured in that node's own `AddNode.cascaded`, so just skip it.
        if document.scenery().node_recursive(uuid).is_err() {
            continue;
        }
        let (node_inverse, response) = delete_node_capturing(&mut document, uuid)?;
        // Prepend each node's inverse so the batch ends up in reverse deletion order: on undo a node's
        // captured connections only reference nodes that were still alive when it was deleted (i.e.
        // deleted later, or not at all), so restoring later-deleted nodes first guarantees every
        // reconnect target already exists.
        let mut combined = node_inverse;
        combined.append(&mut inverse);
        inverse = combined;

        merged.deleted_nodes.extend(response.deleted_nodes);
        merged
            .disconnected_connections
            .extend(response.disconnected_connections);
        merged
            .removed_port_mappings
            .extend(response.removed_port_mappings);
    }
    drop(document);

    push_delete_inverse(&data, inverse);
    Ok(web::Json(merged))
}

/// Pushes the inverse commands captured by [`delete_node_capturing`] as one undo entry: nothing if
/// empty, the single command directly if there is exactly one, otherwise a [`Command::Batch`].
fn push_delete_inverse(data: &AppState, inverse: Vec<Command>) {
    match inverse.len() {
        0 => {}
        1 => data.push_undo(inverse.into_iter().next().unwrap()),
        _ => data.push_undo(Command::Batch(inverse)),
    }
}

/// Deletes `uuid` from `document` and returns `(inverse, response)`: the commands that undo the
/// deletion (in application order - `AddNode` first, so the node exists before its ports are
/// re-mapped or its analyzer mappings restored) and the [`DeleteNodeResponse`] describing what was
/// torn down. Does not push anything onto the undo stack - the caller decides whether this is its own
/// undo step ([`delete_node`]) or part of a batch ([`delete_nodes`]).
///
/// # Errors
///
/// Returns an error if `uuid` doesn't resolve to a node, or a cascade/removal step fails.
fn delete_node_capturing(
    document: &mut OpmDocument,
    uuid: Uuid,
) -> Result<(Vec<Command>, DeleteNodeResponse), BackEndErrorResponse> {
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
                // `find_all_nodes_referring_to_uuid` reports the queried node itself as one of its own
                // "referrers" - skip that self-match (same as the cut path in `operations.rs`), or the
                // target node ends up in `cascaded` too and undo re-adds it twice (a duplicate uuid in
                // the scenery, which crashes the analyzer editor's keyed source-port list).
                if *ref_id == uuid {
                    continue;
                }
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
    let mut removed_port_cascades =
        disconnect_exposed_port_cascades_for_node(scenery, parent_group_id, uuid)?;
    for (member_parent, member_ref) in &cascaded {
        if let Ok(member_uuid) = member_ref.uuid() {
            removed_port_cascades.extend(disconnect_exposed_port_cascades_for_node(
                scenery,
                *member_parent,
                member_uuid,
            )?);
        }
    }
    let deleted_nodes = scenery.delete_node(uuid)?;

    // Deleting a source-port node strips its mapping from every analyzer - capture the inverse commands
    // that restore those mappings, so undo isn't a silent data loss (see the helper).
    let analyzer_inverses = prune_analyzer_source_mappings(document, &deleted_nodes);

    // Only claim cascaded nodes that `delete_node` actually removed, in case its cascade rules ever
    // diverge from what `find_all_nodes_referring_to_uuid` predicted.
    let cascaded: Vec<(Uuid, OpticRef)> = cascaded
        .into_iter()
        .filter(|(_, r)| r.uuid().is_ok_and(|id| deleted_nodes.contains(&id)))
        .collect();
    let (disconnected_connections, removed_port_mappings) =
        split_cascades_for_response(&removed_port_cascades);

    // AddNode first (the node must exist before its ports can be re-mapped), then one restore command
    // per cascade (each an inner batch: AddPortMap per level innermost-first, then the AddEdge that
    // reconnects the terminal external connection), then the analyzer-mapping restores.
    let mut inverse = vec![Command::AddNode(NodeSnapshot {
        parent_group_id,
        node: target_ref,
        cascaded,
        connections,
    })];
    inverse.extend(removed_port_cascades.iter().map(Command::from));
    inverse.extend(analyzer_inverses);

    Ok((
        inverse,
        DeleteNodeResponse {
            deleted_nodes,
            disconnected_connections,
            removed_port_mappings,
        },
    ))
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

    let new_ref_uuid = document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |g| g.add_node(node_reference.clone()))??;

    // Capture the inserted reference node's live `OpticRef` so undo can restore it exactly, mirroring
    // `post_children`. A reference node has neither a cascade nor its own connections at creation time,
    // so both are empty; its *deletion* is already covered symmetrically by `delete_node`'s `AddNode`.
    let node_ref = document.scenery().node_recursive(new_ref_uuid)?.0;
    drop(document);
    data.push_undo(Command::RemoveNode(NodeSnapshot {
        parent_group_id: group_uuid,
        node: node_ref,
        cascaded: Vec::new(),
        connections: Vec::new(),
    }));

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
    use opossum_core::{
        millimeter,
        nodes::Dummy,
        types::api_types::{DocumentChange, NodeEditorPanel, UndoRedoResponse},
        utils::geom_transformation::Isometry,
    };

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

    /// Regression test for `panel_for_update`'s panel-attribution priority, which drives the GUI's
    /// auto-select-and-open-panel feature on undo/redo: `DocumentChange::NodePatched.panel` must name
    /// the correct node-editor sidebar panel (or `None`) for every field combination `PATCH
    /// /api/nodes/{uuid}` can send.
    #[actix_web::test]
    async fn test_patch_node_reports_correct_panel() {
        let app_state = Data::new(AppState::default());
        let node_id = {
            let mut document = app_state.document.lock();
            document.scenery_mut().add_node(Dummy::default()).unwrap()
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(patch_node)
                .service(undo_document),
        )
        .await;

        macro_rules! patch_and_undo {
            ($request:expr) => {{
                let req = test::TestRequest::patch()
                    .uri(&format!("/{node_id}"))
                    .set_json(&$request)
                    .to_request();
                assert_eq!(
                    app.call(req).await.unwrap().status(),
                    StatusCode::NO_CONTENT
                );
                let req = test::TestRequest::post().uri("/undo").to_request();
                let resp = app.call(req).await.unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                let body: UndoRedoResponse = test::read_body_json(resp).await;
                body
            }};
        }

        let iso = Isometry::new_along_z(millimeter!(10.0)).unwrap();

        let body = patch_and_undo!(UpdateNodeRequest {
            name: Some("renamed".to_string()),
            ..Default::default()
        });
        assert!(
            matches!(
                &body.changes[0],
                DocumentChange::NodePatched {
                    panel: Some(NodeEditorPanel::General),
                    ..
                }
            ),
            "a name-only patch must report the General panel"
        );

        let body = patch_and_undo!(UpdateNodeRequest {
            inverted: Some(true),
            ..Default::default()
        });
        assert!(
            matches!(
                &body.changes[0],
                DocumentChange::NodePatched {
                    panel: Some(NodeEditorPanel::General),
                    ..
                }
            ),
            "an inverted-only patch must report the General panel"
        );

        let body = patch_and_undo!(UpdateNodeRequest {
            isometry: Some(Some(iso)),
            ..Default::default()
        });
        assert!(
            matches!(
                &body.changes[0],
                DocumentChange::NodePatched {
                    panel: Some(NodeEditorPanel::Positioning),
                    ..
                }
            ),
            "an isometry-only patch must report the Positioning panel"
        );

        let body = patch_and_undo!(UpdateNodeRequest {
            alignment: Some(Some(iso)),
            ..Default::default()
        });
        assert!(
            matches!(
                &body.changes[0],
                DocumentChange::NodePatched {
                    panel: Some(NodeEditorPanel::Alignment),
                    ..
                }
            ),
            "an alignment-only patch must report the Alignment panel"
        );

        let body = patch_and_undo!(UpdateNodeRequest {
            isometry: Some(Some(iso)),
            alignment: Some(Some(iso)),
            ..Default::default()
        });
        assert!(
            matches!(
                &body.changes[0],
                DocumentChange::NodePatched {
                    panel: Some(NodeEditorPanel::Alignment),
                    ..
                }
            ),
            "when both isometry and alignment are set, alignment must win the tie-break"
        );

        let body = patch_and_undo!(UpdateNodeRequest {
            gui_position: Some(Some((1.0, 2.0))),
            ..Default::default()
        });
        assert!(
            matches!(
                &body.changes[0],
                DocumentChange::NodePatched { panel: None, .. }
            ),
            "a gui_position-only patch (a canvas drag) must report no panel"
        );
    }

    /// Regression test for the bug where `PATCH /api/nodes/{uuid}` always 400ed when `uuid` named the
    /// scenery root itself. Root cause: the handler derived `parent_group_id` via
    /// `NodeGroup::node_recursive`, which only ever searches a group's *children* for a matching uuid -
    /// so it could never succeed for the root's own uuid, since the root is never a child of itself.
    /// This fired on every GUI startup, since the GUI renames the root scenery tab to match the
    /// project's file name right after mounting. Fixed via `parent_group_id_or_self`
    /// (`helper_functions.rs`), which reports the root's own uuid as its "parent" - the same
    /// self-as-parent sentinel `remove_port_map_cascade` already uses for the same reason. Patches the
    /// scenery root's own name and asserts success (204), not the previous 400.
    #[actix_web::test]
    async fn test_patch_node_on_scenery_root_succeeds() {
        let app_state = Data::new(AppState::default());
        let root_id = app_state.document.lock().scenery().node_attr().uuid();

        let app =
            test::init_service(App::new().app_data(app_state.clone()).service(patch_node)).await;

        let req = test::TestRequest::patch()
            .uri(&format!("/{root_id}"))
            .set_json(&UpdateNodeRequest {
                name: Some("renamed".to_string()),
                ..Default::default()
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NO_CONTENT,
            "patching the scenery root itself must succeed, not 400"
        );
        assert_eq!(
            app_state.document.lock().scenery().node_attr().name(),
            "renamed"
        );
    }

    /// Regression test for the bug where Undo was available right after creating a new (empty)
    /// project: the GUI renames the scenery root to the project's file name (or "unsaved")
    /// directly after `DELETE /api/document` cleared the undo history, and `patch_node`
    /// unconditionally pushed an undo entry for that programmatic rename - so Ctrl+Z on a fresh
    /// project would "undo" internal bookkeeping. A root patch must apply without becoming an
    /// undo step, while a patch of an ordinary node must still be undoable.
    #[actix_web::test]
    async fn test_patch_scenery_root_is_not_undoable() {
        let app_state = Data::new(AppState::default());
        let (root_id, node_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let node_id = document.scenery_mut().add_node(Dummy::default()).unwrap();
            (root_id, node_id)
        };

        let app =
            test::init_service(App::new().app_data(app_state.clone()).service(patch_node)).await;

        let req = test::TestRequest::patch()
            .uri(&format!("/{root_id}"))
            .set_json(&UpdateNodeRequest {
                name: Some("unsaved".to_string()),
                ..Default::default()
            })
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            app_state.document.lock().scenery().node_attr().name(),
            "unsaved",
            "the root rename must still be applied"
        );
        assert!(
            app_state.undo_stack.lock().is_empty(),
            "renaming the scenery root is programmatic bookkeeping and must not become an undo step"
        );

        // Guard against over-suppression: patching an ordinary node must still push an undo entry.
        let req = test::TestRequest::patch()
            .uri(&format!("/{node_id}"))
            .set_json(&UpdateNodeRequest {
                name: Some("user renamed".to_string()),
                ..Default::default()
            })
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            app_state.undo_stack.lock().len(),
            1,
            "a normal node rename must remain undoable"
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

    /// Regression test for the bug where deleting a node whose port was externally mapped left a
    /// dangling connection behind: `OpticGraph::delete_node` correctly prunes the port-map *entry*
    /// pointing at the deleted node, but the *external connection* that used that mapping - a separate
    /// edge one level up, in the mapping group's own parent graph - was never touched. Builds a group
    /// `G` containing node `A`, maps `A`'s input to `G`'s external port `ext_in_1`, connects a sibling
    /// `S` (in the root, `G`'s parent) to `G:ext_in_1`, deletes `A`, and asserts the `S -> G` connection
    /// is gone. One undo must restore `A`, the port mapping, and the `S -> G` connection together.
    ///
    /// Also covers the follow-up bug where `G`'s own displayed port handle (as seen from `G`'s
    /// parent) stayed visible after the deletion: the response's `removed_port_mappings` used to
    /// carry only `(group_id, node_id)`, not enough for the GUI to call the handler that shrinks a
    /// group's own port handles (`remove_group_port`, which needs the external port name + type) -
    /// asserts the response now carries that name and type too.
    #[actix_web::test]
    async fn test_undo_delete_mapped_node_restores_port_map_and_external_connection() {
        use opossum_core::{
            meter,
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (root_id, group_id, node_a, sibling_s) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let node_a = group.add_node(Dummy::default()).unwrap();
            group.map_input_port(node_a, "input_1", "ext_in_1").unwrap();
            let group_id = scenery.add_node(group).unwrap();

            let sibling_s = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(sibling_s, "output_1", group_id, "ext_in_1", meter!(0.1))
                .unwrap();

            (root_id, group_id, node_a, sibling_s)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(delete_node)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/{node_a}"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response: DeleteNodeResponse = test::read_body_json(resp).await;
        assert!(
            response
                .disconnected_connections
                .iter()
                .any(|(group_id_, c)| *group_id_ == root_id
                    && c.src_uuid() == sibling_s
                    && c.target_uuid() == group_id),
            "the response must report the disconnected S -> G connection"
        );
        assert_eq!(
            response.removed_port_mappings,
            vec![(group_id, node_a, "ext_in_1".to_string(), PortType::Input)],
            "the response must carry the external port name and type, so the GUI can shrink \
             G's own displayed port handle instead of leaving it stale"
        );

        {
            let document = app_state.document.lock();
            let connections = document
                .scenery()
                .with_group_node(root_id, NodeGroup::connections)
                .unwrap();
            assert!(
                !connections
                    .iter()
                    .any(|c| c.src_id == sibling_s && c.target_id == group_id),
                "the dangling S -> G connection must be gone once A (and its mapping) is deleted"
            );
        }

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "undo must not error");

        let document = app_state.document.lock();
        assert!(
            document.scenery().node_recursive(node_a).is_ok(),
            "node A must be restored"
        );
        let restored_mapping = document
            .scenery()
            .with_group_node(group_id, |g| {
                g.graph()
                    .port_map(&PortType::Input)
                    .get("ext_in_1")
                    .cloned()
            })
            .unwrap();
        assert_eq!(
            restored_mapping,
            Some((node_a, "input_1".to_string())),
            "the port mapping must be restored"
        );
        let connections = document
            .scenery()
            .with_group_node(root_id, NodeGroup::connections)
            .unwrap();
        assert!(
            connections.iter().any(|c| c.src_id == sibling_s
                && c.target_id == group_id
                && c.target_port == "ext_in_1"),
            "the S -> G external connection must be restored"
        );
    }

    /// Regression test for the multi-delete-needs-N-undos bug: selecting several connected nodes and
    /// deleting them in one gesture used to fire one `DELETE /{uuid}` per node, so each became its own
    /// undo step. `POST /delete` deletes the whole selection as a single undo step: one undo must
    /// restore every node *and* the connection between them.
    #[actix_web::test]
    async fn test_delete_nodes_batch_is_one_undo_step() {
        use opossum_core::{
            meter,
            nodes::{Dummy, NodeGroup},
        };

        let app_state = Data::new(AppState::default());
        let (root_id, node_b, node_c) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();
            let node_b = scenery.add_node(Dummy::default()).unwrap();
            let node_c = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(node_b, "output_1", node_c, "input_1", meter!(0.1))
                .unwrap();
            (root_id, node_b, node_c)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(delete_nodes)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/delete")
            .set_json(&vec![node_b, node_c])
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response: DeleteNodeResponse = test::read_body_json(resp).await;
        assert!(
            response.deleted_nodes.contains(&node_b) && response.deleted_nodes.contains(&node_c),
            "both nodes must be reported deleted, got {:?}",
            response.deleted_nodes
        );
        assert_eq!(
            app_state.undo_stack.lock().len(),
            1,
            "deleting a multi-node selection must be a single undo step, not one per node"
        );

        // One undo restores both nodes and the connection between them.
        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "a single undo must restore the whole selection"
        );
        let document = app_state.document.lock();
        assert!(
            document.scenery().node_recursive(node_b).is_ok()
                && document.scenery().node_recursive(node_c).is_ok(),
            "both nodes must be restored by one undo"
        );
        let connections = document
            .scenery()
            .with_group_node(root_id, NodeGroup::connections)
            .unwrap();
        assert!(
            connections
                .iter()
                .any(|c| c.src_id == node_b && c.target_id == node_c),
            "the connection between the two deleted nodes must be restored too"
        );
    }

    /// Regression test for the serious bug where deleting a node inside a *doubly*-nested group did
    /// not cascade its port-map teardown outward: `root -> G1 -> G2 -> B`, where B's input is exposed
    /// on G2, re-exposed on G1, and finally consumed by a live connection `S -> G1` at the root.
    /// The old single-hop teardown only looked one level up from G2 (at G1), found no live connection
    /// there (G1 re-exposes rather than consuming), and gave up - leaving G1's mapping and the root
    /// connection dangling. Deleting B must now remove *both* mapping levels and the root connection,
    /// report both levels, and undo must restore the node, both mappings, and the connection.
    #[actix_web::test]
    async fn test_undo_delete_doubly_nested_mapped_node_cascades_outward() {
        use opossum_core::{
            meter,
            nodes::{Dummy, NodeGroup},
            prelude::PortType,
        };

        let app_state = Data::new(AppState::default());
        let (root_id, g1_id, g2_id, node_b, sibling_s) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            // Innermost group G2 exposes B's input as "g2_in".
            let mut g2 = NodeGroup::new("G2");
            let node_b = g2.add_node(Dummy::default()).unwrap();
            g2.map_input_port(node_b, "input_1", "g2_in").unwrap();

            // G1 contains G2 and re-exposes "g2_in" as its own "g1_in".
            let mut g1 = NodeGroup::new("G1");
            let g2_id = g1.add_node(g2).unwrap();
            g1.map_input_port(g2_id, "g2_in", "g1_in").unwrap();

            // Root contains G1 and a sibling S wired into G1's "g1_in".
            let g1_id = scenery.add_node(g1).unwrap();
            let sibling_s = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(sibling_s, "output_1", g1_id, "g1_in", meter!(0.1))
                .unwrap();

            (root_id, g1_id, g2_id, node_b, sibling_s)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(delete_node)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/{node_b}"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response: DeleteNodeResponse = test::read_body_json(resp).await;

        // Both cascade levels must be reported (innermost G2 first, then G1).
        assert!(
            response.removed_port_mappings.contains(&(
                g2_id,
                node_b,
                "g2_in".to_string(),
                PortType::Input
            )),
            "G2's own mapping of B must be reported removed, got {:?}",
            response.removed_port_mappings
        );
        assert!(
            response.removed_port_mappings.contains(&(
                g1_id,
                g2_id,
                "g1_in".to_string(),
                PortType::Input
            )),
            "G1's re-exposed mapping must be reported removed too, got {:?}",
            response.removed_port_mappings
        );
        assert!(
            response
                .disconnected_connections
                .iter()
                .any(|(gid, c)| *gid == root_id
                    && c.src_uuid() == sibling_s
                    && c.target_uuid() == g1_id),
            "the terminal S -> G1 connection must be reported disconnected, got {:?}",
            response.disconnected_connections
        );

        // Live state: both mappings gone, root connection gone.
        {
            let document = app_state.document.lock();
            let g1_mapping = document
                .scenery()
                .with_group_node(g1_id, |g| {
                    g.graph().port_map(&PortType::Input).get("g1_in").cloned()
                })
                .unwrap();
            assert!(
                g1_mapping.is_none(),
                "G1's mapping must be gone once B (deep inside) is deleted - this is the bug"
            );
            let connections = document
                .scenery()
                .with_group_node(root_id, NodeGroup::connections)
                .unwrap();
            assert!(
                !connections
                    .iter()
                    .any(|c| c.src_id == sibling_s && c.target_id == g1_id),
                "the dangling S -> G1 connection must be gone"
            );
        }

        // Undo restores the node, both mapping levels, and the root connection.
        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::OK,
            "undo must not error"
        );
        let document = app_state.document.lock();
        assert!(
            document.scenery().node_recursive(node_b).is_ok(),
            "node B must be restored"
        );
        let g2_mapping = document
            .scenery()
            .with_group_node(g2_id, |g| {
                g.graph().port_map(&PortType::Input).get("g2_in").cloned()
            })
            .unwrap();
        assert_eq!(
            g2_mapping,
            Some((node_b, "input_1".to_string())),
            "G2's mapping of B must be restored"
        );
        let g1_mapping = document
            .scenery()
            .with_group_node(g1_id, |g| {
                g.graph().port_map(&PortType::Input).get("g1_in").cloned()
            })
            .unwrap();
        assert_eq!(
            g1_mapping,
            Some((g2_id, "g2_in".to_string())),
            "G1's re-exposed mapping must be restored"
        );
        let connections = document
            .scenery()
            .with_group_node(root_id, NodeGroup::connections)
            .unwrap();
        assert!(
            connections
                .iter()
                .any(|c| c.src_id == sibling_s && c.target_id == g1_id && c.target_port == "g1_in"),
            "the S -> G1 external connection must be restored"
        );
    }

    /// Regression test for the bug where renaming a node that has a reference took TWO undos: the GUI
    /// fanned the rename out into one PATCH per node (original + each reference), each its own undo
    /// step. The backend now propagates the rename to reference nodes itself and records it as a
    /// single `Command::Batch`, so one undo restores both names.
    #[actix_web::test]
    async fn test_rename_with_reference_is_one_undo_step() {
        use crate::document::undo_document;

        let app_state = Data::new(AppState::default());
        let (root_id, node_a, original_name) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let node_a = document.scenery_mut().add_node(Dummy::default()).unwrap();
            let original_name = document
                .scenery()
                .with_node_attr(node_a, |a| a.name().to_string())
                .unwrap();
            (root_id, node_a, original_name)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_reference)
                .service(patch_node)
                .service(undo_document),
        )
        .await;

        // Add a reference to A - its name is `ref (<A's name>)`.
        let req = test::TestRequest::post()
            .uri(&format!("/{root_id}/references"))
            .set_json(&NewRefNode::new(node_a, (10.0, 20.0)))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let ref_info: NodeInfo = test::read_body_json(resp).await;
        let ref_id = ref_info.uuid();

        // Rename A - this must propagate to the reference and add exactly one undo step (creating the
        // reference already pushed one, so measure the growth rather than the absolute stack size).
        let undo_len_before = app_state.undo_stack.lock().len();
        let req = test::TestRequest::patch()
            .uri(&format!("/{node_a}"))
            .set_json(&UpdateNodeRequest {
                name: Some("renamed".to_string()),
                ..Default::default()
            })
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let read_name = |id| {
            app_state
                .document
                .lock()
                .scenery()
                .with_node_attr(id, |a| a.name().to_string())
                .unwrap()
        };
        assert_eq!(read_name(node_a), "renamed");
        assert_eq!(
            read_name(ref_id),
            "ref (renamed)",
            "the rename must propagate to the reference node"
        );
        assert_eq!(
            app_state.undo_stack.lock().len(),
            undo_len_before + 1,
            "renaming a node with a reference must add exactly one undo step, not one per node"
        );

        // One undo restores both names.
        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);
        assert_eq!(read_name(node_a), original_name);
        assert_eq!(
            read_name(ref_id),
            format!("ref ({original_name})"),
            "one undo must restore the reference's name too"
        );
    }

    /// Regression test for the gap where creating a reference node (`POST /{uuid}/references`) pushed
    /// no undo command at all - so a freshly added reference node could not be removed via undo, even
    /// though *deleting* one is already covered by `delete_node`'s `AddNode`. Adds a reference node
    /// pointing at a dummy, undoes the creation (node must be gone), and redoes it (node must reappear
    /// under the same uuid).
    #[actix_web::test]
    async fn test_undo_redo_add_reference_node() {
        use crate::document::{redo_document, undo_document};

        let app_state = Data::new(AppState::default());
        let (root_id, target_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let target_id = document.scenery_mut().add_node(Dummy::default()).unwrap();
            (root_id, target_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_reference)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/{root_id}/references"))
            .set_json(&NewRefNode::new(target_id, (10.0, 20.0)))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let ref_info: NodeInfo = test::read_body_json(resp).await;
        let ref_uuid = ref_info.uuid();
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(ref_uuid)
                .is_ok(),
            "the reference node must exist right after creation"
        );

        // Undo removes the just-created reference node.
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(ref_uuid)
                .is_err(),
            "undo must remove the reference node"
        );

        // Redo restores it under the same uuid.
        let req = test::TestRequest::post().uri("/redo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(ref_uuid)
                .is_ok(),
            "redo must restore the reference node under the same uuid"
        );
    }

    /// Regression test for the cascade gap where *adding* a `"source port"` node injected a default
    /// mapping into every analyzer, but the undo (a bare `RemoveNode`) only removed the node - leaving a
    /// dangling mapping to a nonexistent uuid behind. Sets up an (empty) Energy analyzer, adds a source
    /// port via `post_children` (which injects the mapping), and asserts a single undo removes both the
    /// node and the injected analyzer mapping.
    #[actix_web::test]
    async fn test_undo_add_source_port_removes_injected_analyzer_mapping() {
        use crate::document::undo_document;
        use opossum_core::prelude::{AnalyzerType, EnergyConfig};

        let app_state = Data::new(AppState::default());
        let (root_id, analyzer_id, empty_type) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let analyzer_id = document
                .add_analyzer_with_position(AnalyzerType::Energy(EnergyConfig::default()), None);
            let empty_type = document
                .analyzer(analyzer_id)
                .unwrap()
                .analyzer_type()
                .clone();
            (root_id, analyzer_id, empty_type)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_children)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/{root_id}/children"))
            .set_json(&NewNode::new("source port".to_string(), (0.0, 0.0)))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let src_info: NodeInfo = test::read_body_json(resp).await;
        let src_uuid = src_info.uuid();

        assert_ne!(
            *app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .analyzer_type(),
            empty_type,
            "adding the source port must have injected a mapping into the analyzer"
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        {
            let document = app_state.document.lock();
            assert!(
                document.scenery().node_recursive(src_uuid).is_err(),
                "undo must remove the source-port node"
            );
            assert_eq!(
                *document.analyzer(analyzer_id).unwrap().analyzer_type(),
                empty_type,
                "undo must also strip the analyzer mapping the add injected, not leave it dangling"
            );
        }
    }

    /// Regression test for the sharper direction of the same gap: *deleting* a mapped `"source port"`
    /// node stripped its mapping from every analyzer, but the undo (an `AddNode`) restored only the
    /// node, silently losing the analyzer's source mapping. Builds a source port already mapped in an
    /// Energy analyzer, deletes it (mapping must be gone), and asserts one undo restores BOTH the node
    /// and the analyzer mapping.
    #[actix_web::test]
    async fn test_undo_delete_source_port_restores_analyzer_mapping() {
        use crate::document::undo_document;
        use opossum_core::{
            light::lightdata::energy_data_builder::EnergyDataBuilder,
            nodes::create_node_ref,
            prelude::{AnalyzerType, EnergyConfig},
        };

        let app_state = Data::new(AppState::default());
        let (src_uuid, analyzer_id, mapped_type) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();

            let src_ref = create_node_ref("source port").unwrap();
            let src_uuid = src_ref.uuid().unwrap();
            document
                .scenery_mut()
                .with_group_node_mut(root_id, |g| g.add_node_ref(src_ref))
                .unwrap()
                .unwrap();

            let mut cfg = EnergyConfig::default();
            cfg.map_source(src_uuid, EnergyDataBuilder::default());
            let analyzer_id = document.add_analyzer_with_position(AnalyzerType::Energy(cfg), None);
            let mapped_type = document
                .analyzer(analyzer_id)
                .unwrap()
                .analyzer_type()
                .clone();
            (src_uuid, analyzer_id, mapped_type)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(delete_node)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/{src_uuid}"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_ne!(
            *app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .analyzer_type(),
            mapped_type,
            "deleting the source port must have removed its analyzer mapping"
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "undo must not error");
        {
            let document = app_state.document.lock();
            assert!(
                document.scenery().node_recursive(src_uuid).is_ok(),
                "undo must restore the source-port node"
            );
            assert_eq!(
                *document.analyzer(analyzer_id).unwrap().analyzer_type(),
                mapped_type,
                "undo must also restore the analyzer source mapping, not just the node"
            );
        }
    }

    /// Regression test for the live-QA crash: analyzer present, create a `"source port"`, delete it,
    /// undo - the analyzer editor then listed the source port *twice* and crashed Dioxus with a
    /// duplicate-key panic. The editor's list is fed by `get_available_sources` (a scenery walk), so a
    /// duplicate there means the scenery holds two nodes with the same source-port uuid after undo.
    /// Mirrors the exact flow (`post_children` source port, `delete_node`, `/undo`) and asserts exactly
    /// one source port is visible afterwards.
    #[actix_web::test]
    async fn test_undo_delete_source_port_yields_exactly_one_source() {
        use crate::{analyzers::get_available_sources, document::undo_document};
        use opossum_core::{
            prelude::{AnalyzerType, EnergyConfig},
            types::api_types::SourcePortDto,
        };

        let app_state = Data::new(AppState::default());
        let root_id = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            // An analyzer exists first, matching the user's flow.
            document
                .add_analyzer_with_position(AnalyzerType::Energy(EnergyConfig::default()), None);
            root_id
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_children)
                .service(delete_node)
                .service(undo_document)
                .service(get_available_sources),
        )
        .await;

        // Create the source port (post_children injects it into the analyzer).
        let req = test::TestRequest::post()
            .uri(&format!("/{root_id}/children"))
            .set_json(&NewNode::new("source port".to_string(), (0.0, 0.0)))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let src: NodeInfo = test::read_body_json(resp).await;
        let src_uuid = src.uuid();

        let count_sources = |state: &Data<AppState>| -> usize {
            let document = state.document.lock();
            let root = document.scenery().node_attr().uuid();
            document
                .scenery()
                .with_group_node(root, |g| {
                    g.nodes()
                        .iter()
                        .filter(|n| {
                            n.optical_ref
                                .lock_opm()
                                .map_or(false, |node| node.node_attr().node_type() == "source port")
                        })
                        .count()
                })
                .unwrap()
        };
        assert_eq!(count_sources(&app_state), 1, "one source after create");

        // Delete it.
        let req = test::TestRequest::delete()
            .uri(&format!("/{src_uuid}"))
            .to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);
        assert_eq!(count_sources(&app_state), 0, "zero sources after delete");

        // Undo the delete.
        let req = test::TestRequest::post().uri("/undo").to_request();
        assert_eq!(app.call(req).await.unwrap().status(), StatusCode::OK);
        assert_eq!(count_sources(&app_state), 1, "one source after undo");

        // Exactly one source port must be visible - not two.
        let req = test::TestRequest::get()
            .uri("/available_sources")
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let sources: Vec<SourcePortDto> = test::read_body_json(resp).await;
        assert_eq!(
            sources.len(),
            1,
            "after delete+undo there must be exactly one source port, got {sources:?}"
        );
        assert_eq!(sources[0].uuid, src_uuid);
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

/// Removes the source mappings of every just-deleted node from all analyzers (deleting a `"source port"`
/// node must strip it from each analyzer's source map), returning the `PatchAnalyzer` inverse commands
/// that restore each changed analyzer's mapping on undo. Each returned command's own inverse re-prunes on
/// redo, so folding these into the delete's undo batch keeps the whole cascade reversible.
///
/// # Arguments
///
/// - `document`: the live document whose analyzers are pruned in place.
/// - `deleted_nodes`: the uuids that `delete_node` just removed (target plus any cascaded reference nodes).
///
/// # Returns
///
/// One `Command::PatchAnalyzer` per analyzer whose config actually changed; empty if none did.
fn prune_analyzer_source_mappings(
    document: &mut OpmDocument,
    deleted_nodes: &[Uuid],
) -> Vec<Command> {
    // Snapshot each analyzer's config *before* pruning (indexed by id for lookup afterward), so undo can
    // restore whatever the prune below removes.
    let old_analyzer_types: HashMap<Uuid, AnalyzerType> = document
        .analyzers()
        .iter()
        .map(|(id, info)| (*id, info.analyzer_type().clone()))
        .collect();

    for deleted_uuid in deleted_nodes {
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

    let mut inverses = Vec::new();
    for (az_uuid, old_type) in &old_analyzer_types {
        if let Ok(info) = document.analyzer(*az_uuid) {
            let new_type = info.analyzer_type().clone();
            if new_type != *old_type {
                inverses.push(Command::PatchAnalyzer(PatchAnalyzer {
                    id: *az_uuid,
                    old: new_type,
                    new: old_type.clone(),
                }));
            }
        }
    }
    inverses
}
