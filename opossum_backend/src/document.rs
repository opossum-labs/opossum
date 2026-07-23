//! Routes for managing the document
use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    sse_logger::SENDER,
    undo::{Command, PatchNode, RepositionAnalyzer, capture_old_node_request},
};
use actix_web::{
    Error, HttpResponse, Responder, delete, get, patch, post, put,
    web::{self, Json},
};
use futures_util::StreamExt;
use log::{error, info, warn};
use opossum_core::{
    core_optics::{SceneryResources, node_attr::HasNodeAttr},
    opm_document::OpmDocument,
    types::api_types::{
        ErrorResponse, LoadDocumentResponse, PositionUpdate, UndoRedoResponse, UpdateNodeRequest,
    },
};
use std::{path::PathBuf, str::FromStr};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use utoipa_actix_web::service_config::ServiceConfig;

const RON_MEDIA_TYPE: &str = "application/ron";

/// Delete the current document and create new (empty) one
#[utoipa::path(responses((status = NO_CONTENT, description = "document deleted and new one sucessfully created")), tag="document")]
#[delete("")]
async fn delete_document(data: web::Data<AppState>) -> impl Responder {
    let mut document = data.document.lock();
    *document = OpmDocument::default();
    drop(document);
    data.clear_undo_history();
    HttpResponse::NoContent().finish()
}
#[utoipa::path(tag = "document",
    responses((status = 200, description = "Global configuration", body = SceneryResources))
)]
/// Get the global configuration of this model
///
/// This function returns the global configuration of the model.
#[get("/global_conf")]
async fn get_global_conf(data: web::Data<AppState>) -> impl Responder {
    let document = data.document.lock();
    web::Json(document.global_conf().lock().unwrap().clone())
}

#[utoipa::path(tag = "document",
    responses((status = 200, description = "Global configuration", body = SceneryResources))
)]
/// Set the global configuration
///
/// This function sets the global configuration of the model. The old global configuration is
/// replaced by the new one.
#[patch("/global_conf")]
async fn patch_global_conf(
    data: web::Data<AppState>,
    new_global_conf: web::Json<SceneryResources>,
) -> impl Responder {
    let global_conf = new_global_conf.into_inner();
    data.document.lock().set_global_conf(global_conf.clone());
    web::Json(global_conf)
}
#[utoipa::path(tag = "document",
    responses((status = 200, description = "Scenery Uuid", body = SceneryResources))
)]
/// Get the uuid of the root node of this model
///
/// This function returns the uuid of the root node (group) of the document.
#[get("/root_uuid")]
async fn get_root_uuid(data: web::Data<AppState>) -> impl Responder {
    let document = data.document.lock();
    web::Json(document.scenery().node_attr().uuid())
}
/// Get the document as an (OPM file) string
///
/// This function returns the entire document as an OPM model file string.
#[utoipa::path(tag = "document", 
    responses((status = 200, description = "OPM file", body = String, content_type=RON_MEDIA_TYPE))
)]
#[get("")]
async fn get_document(data: web::Data<AppState>) -> Result<impl Responder, BackEndErrorResponse> {
    let document = data.document.lock();
    Ok(HttpResponse::Ok()
        .content_type(RON_MEDIA_TYPE)
        .body(document.to_opm_file_string()?))
}
#[utoipa::path(
    tag = "document", 
    request_body(
        content = String,
        description = "OPM file as string",
        content_type = "text/plain",
    ),
    responses(
        // Hier wurde 'body = LoadDocumentResponse' hinzugefügt
        (status = 200, description = "OPM file successfully parsed", body = LoadDocumentResponse),
        (status = 400, description = "Error parsing OPM file", body = ErrorResponse)
    )
)]
/// Load a document from an OPM file string
///
/// This function reads a OPM model from the given OPM file string and replaces the current
/// document. It also evaluates if the newly loaded document is missing GUI coordinates.
#[put("")]
async fn put_document(
    data: web::Data<AppState>,
    opm_file_string: String,
) -> Result<Json<LoadDocumentResponse>, BackEndErrorResponse> {
    let mut document = data.document.lock();
    *document = OpmDocument::from_string(&opm_file_string)?;

    let name = document.scenery().node_attr().name().to_string();
    // Check if the graph is missing GUI coordinates using the method we defined earlier
    let needs_autolayout = document.needs_autolayout();

    drop(document);
    data.clear_undo_history();

    Ok(Json(LoadDocumentResponse {
        name,
        needs_autolayout,
    }))
}

/// Undo the last checkpointed document edit.
///
/// Pops the most recent entry off the undo history, reverses it, and pushes its own inverse onto
/// the redo history. Returns the concrete changes this made (so the GUI can update its canvas state
/// directly, the same way it reacts to a normal edit) plus the resulting undo/redo availability.
#[utoipa::path(tag = "document",
    responses(
        (status = OK, description = "Undo applied", body = UndoRedoResponse),
        (status = 409, description = "Nothing to undo", body = ErrorResponse)
    )
)]
#[post("/undo")]
pub(crate) async fn undo_document(
    data: web::Data<AppState>,
) -> Result<Json<UndoRedoResponse>, BackEndErrorResponse> {
    let Some(command) = data.undo_stack.lock().pop_back() else {
        return Err(BackEndErrorResponse::new(409, "Opossum", "Nothing to undo"));
    };
    let changes = command.describe()?;
    let mut document = data.document.lock();
    let inverse = apply_with_rollback(&mut document, command)?;
    drop(document);
    data.redo_stack.lock().push_back(inverse);

    Ok(Json(UndoRedoResponse {
        changes,
        can_undo: !data.undo_stack.lock().is_empty(),
        can_redo: true,
    }))
}

/// Redo the last undone document edit.
///
/// Symmetric to [`undo_document`]: pops from the redo history, applies it, and pushes its inverse
/// back onto the undo history.
#[utoipa::path(tag = "document",
    responses(
        (status = OK, description = "Redo applied", body = UndoRedoResponse),
        (status = 409, description = "Nothing to redo", body = ErrorResponse)
    )
)]
#[post("/redo")]
pub(crate) async fn redo_document(
    data: web::Data<AppState>,
) -> Result<Json<UndoRedoResponse>, BackEndErrorResponse> {
    let Some(command) = data.redo_stack.lock().pop_back() else {
        return Err(BackEndErrorResponse::new(409, "Opossum", "Nothing to redo"));
    };
    let changes = command.describe()?;
    let mut document = data.document.lock();
    let inverse = apply_with_rollback(&mut document, command)?;
    drop(document);
    data.undo_stack.lock().push_back(inverse);

    Ok(Json(UndoRedoResponse {
        changes,
        can_undo: true,
        can_redo: !data.redo_stack.lock().is_empty(),
    }))
}

/// Batch-update the GUI positions of several nodes/analyzers in one step.
///
/// Used at the end of a multi-node drag or after auto-layout, so moving N nodes is one undo step
/// instead of N.
#[utoipa::path(tag = "document",
    request_body(content = Vec<PositionUpdate>, description = "The nodes/analyzers to reposition"),
    responses((status = NO_CONTENT, description = "Positions updated"))
)]
#[patch("/positions")]
async fn patch_positions(
    data: web::Data<AppState>,
    body: web::Json<Vec<PositionUpdate>>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let updates = body.into_inner();
    if updates.is_empty() {
        return Ok(HttpResponse::NoContent().finish());
    }

    let mut document = data.document.lock();
    let backup = document.to_opm_file_string()?;
    let mut inverses = match apply_position_updates(&mut document, updates) {
        Ok(inverses) => inverses,
        Err(err) => {
            // A later update in the batch failed after earlier ones already mutated the document -
            // undo those too, so a partial batch can never leave the document (and the GUI, which
            // only applies changes on a successful response) out of sync.
            *document = OpmDocument::from_string(&backup)?;
            return Err(err);
        }
    };
    inverses.reverse();
    data.push_undo(Command::Batch(inverses));
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

/// Applies each position update to `document` in order, returning the list of inverse commands (one
/// per update, in application order) - or the first error encountered, with `document` left partially
/// mutated by whichever updates ran before it (the caller is expected to restore from a backup).
fn apply_position_updates(
    document: &mut OpmDocument,
    updates: Vec<PositionUpdate>,
) -> Result<Vec<Command>, BackEndErrorResponse> {
    let mut inverses = Vec::with_capacity(updates.len());
    for update in updates {
        let inverse = if update.is_optical {
            let new = UpdateNodeRequest {
                gui_position: Some(Some(update.gui_position)),
                ..Default::default()
            };
            let old = document
                .scenery()
                .with_node_attr(update.uuid, |node_attr| {
                    capture_old_node_request(node_attr, &new)
                })?;
            let parent_group_id = document.scenery().node_recursive(update.uuid)?.1;
            Command::PatchNode(PatchNode {
                uuid: update.uuid,
                parent_group_id,
                old,
                new,
            })
            .apply(document)?
        } else {
            let old_pos = document
                .analyzer_mut(update.uuid)
                .ok_or_else(|| {
                    BackEndErrorResponse::new(404, "Opossum", "UUID not found in analyzers")
                })?
                .gui_position()
                .map_or((0., 0.), |p| (p.x, p.y));
            Command::RepositionAnalyzer(RepositionAnalyzer {
                id: update.uuid,
                old_pos,
                new_pos: update.gui_position,
            })
            .apply(document)?
        };
        inverses.push(inverse);
    }
    Ok(inverses)
}

/// Applies `command` to `document`, restoring `document` to exactly its pre-call state if `apply`
/// fails - so a bug in a multi-step [`Command::apply`] (e.g. a `Batch` or group conversion that fails
/// partway through) can never leave the live document silently torn.
fn apply_with_rollback(
    document: &mut OpmDocument,
    command: Command,
) -> Result<Command, BackEndErrorResponse> {
    let backup = document.to_opm_file_string()?;
    match command.apply(document) {
        Ok(inverse) => Ok(inverse),
        Err(err) => {
            *document = OpmDocument::from_string(&backup)?;
            Err(err)
        }
    }
}

#[utoipa::path(tag = "document", request_body(content = String,
    description = "Start a simulation run",
    content_type = "text/plain",
),
    responses((status = 200, description = "simulation sucessfully performed"))
)]
/// Initiate an OPOSSUM simulation run
///
/// This function starts the simulation of the current document.
#[post("/simulate")]
async fn simulate(data: web::Data<AppState>, report_dir: String) -> impl Responder {
    let (tx, rx) = mpsc::channel(10);
    let mut document = data.document.lock().clone();
    // Run the synchronous, blocking code in a dedicated thread pool.
    web::block(move || {
        SENDER.with(|cell| {
            *cell.borrow_mut() = Some(tx);
        });
        match PathBuf::from_str(&report_dir) {
            Ok(report_dir) => {
                info!("Creating report directory: {}", report_dir.display());
                // if let Err(e) = recreate_data_dir(&report_dir) {
                //     error!("Error creating data directory: {e}");
                // } else {
                info!("Creating diagram files");
                document
                    .create_dot_file(&report_dir)
                    .unwrap_or_else(|e| warn!("{e}"));
                info!("Starting analysis");
                let analysis_reports = document.analyze();
                match analysis_reports {
                    Ok(reports) => {
                        info!("Generating report(s)");
                        for report in reports.iter().enumerate() {
                            report
                                .1
                                .save(&report_dir, report.0)
                                .unwrap_or_else(|e| warn!("{e}"));
                        }
                    }
                    Err(e) => {
                        error!("Error during analysis: {e}");
                    } // }
                }
            }
            Err(e) => {
                error!("Ill-formatted report directory: {e}");
            }
        }
        SENDER.with(|cell| {
            *cell.borrow_mut() = None;
        });
    })
    .await
    .ok(); // We don't care about the result of block, just that it ran.
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(
            ReceiverStream::new(rx).map(|s| -> Result<actix_web::web::Bytes, Error> {
                Ok(actix_web::web::Bytes::from(format!("data: {}\n\n", &s)))
            }),
        )
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_document);
    cfg.service(put_document);
    cfg.service(delete_document);

    cfg.service(get_global_conf);
    cfg.service(patch_global_conf);

    cfg.service(get_root_uuid);

    cfg.service(undo_document);
    cfg.service(redo_document);
    cfg.service(patch_positions);

    // cfg.service(simulate);
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        app_state::AppState,
        undo::{Command, RemoveNode},
    };
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{core_optics::SceneryResources, nodes::create_node_ref};

    #[actix_web::test]
    async fn test_get_global_conf() {
        let app_state = Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(get_global_conf)
                .service(patch_global_conf),
        )
        .await;
        let req = test::TestRequest::get().uri("/global_conf").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let _: SceneryResources = test::read_body_json(resp).await; // Panics, if not valid JSON
    }

    #[actix_web::test]
    async fn test_undo_redo_empty_stack_returns_409() {
        let app_state = Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        let req = test::TestRequest::post().uri("/redo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    /// Mirrors what `nodes/core.rs::delete_node` does: capture a node's `OpticRef` and push its
    /// `AddNode` inverse, i.e. simulates "a node was deleted" for the purposes of this test.
    #[actix_web::test]
    async fn test_undo_redo_restores_node_with_same_uuid() {
        let app_state = Data::new(AppState::default());

        let node_ref = create_node_ref("dummy").unwrap();
        let node_uuid = node_ref.uuid().unwrap();
        let root_id = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            document
                .scenery_mut()
                .with_group_node_mut(root_id, |g| g.add_node_ref(node_ref.clone()))
                .unwrap()
                .unwrap();
            root_id
        };
        app_state.push_undo(Command::RemoveNode(RemoveNode {
            parent_group_id: root_id,
            node: node_ref,
            cascaded: Vec::new(),
            connections: Vec::new(),
        }));
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(node_uuid)
                .is_ok()
        );

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // Undo removes the node.
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(!body.can_undo);
        assert!(body.can_redo);
        assert_eq!(body.changes.len(), 1);
        assert!(matches!(
            &body.changes[0],
            opossum_core::types::api_types::DocumentChange::NodeRemoved { uuid, .. }
                if *uuid == node_uuid
        ));
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(node_uuid)
                .is_err()
        );

        // Redo restores it under the exact same uuid.
        let req = test::TestRequest::post().uri("/redo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(body.can_undo);
        assert!(!body.can_redo);
        assert!(
            app_state
                .document
                .lock()
                .scenery()
                .node_recursive(node_uuid)
                .is_ok()
        );
    }

    #[actix_web::test]
    async fn test_patch_positions_is_one_undo_step_for_multiple_nodes() {
        let app_state = Data::new(AppState::default());

        let (root_id, node_a, node_b) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();
            let a = scenery
                .with_group_node_mut(root_id, |g| {
                    g.add_node_ref(create_node_ref("dummy").unwrap())
                })
                .unwrap()
                .unwrap();
            let b = scenery
                .with_group_node_mut(root_id, |g| {
                    g.add_node_ref(create_node_ref("dummy").unwrap())
                })
                .unwrap()
                .unwrap();
            (root_id, a, b)
        };
        let _ = root_id;

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(patch_positions)
                .service(undo_document),
        )
        .await;

        let updates = vec![
            opossum_core::types::api_types::PositionUpdate {
                uuid: node_a,
                is_optical: true,
                gui_position: (10.0, 20.0),
            },
            opossum_core::types::api_types::PositionUpdate {
                uuid: node_b,
                is_optical: true,
                gui_position: (30.0, 40.0),
            },
        ];
        let req = test::TestRequest::patch()
            .uri("/positions")
            .set_json(&updates)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert_eq!(app_state.undo_stack.lock().len(), 1); // one batch, not two entries

        // One undo reverts both moves at once.
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(!body.can_undo);
        assert_eq!(body.changes.len(), 2);
    }

    /// Regression test for the bug where undoing a group conversion of *connected* nodes silently
    /// dropped the connection between them, and crashed with "target node ... does not exist" if the
    /// group also had a connection to a node outside it. Builds `node_a -> node_b -> node_c`, converts
    /// `{node_a, node_b}` into a group (so `a->b` becomes internal and `b->c` crosses the new group's
    /// boundary), undoes the conversion, and asserts both connections - and the group node itself -
    /// end up exactly as they were before grouping.
    #[actix_web::test]
    async fn test_undo_group_conversion_restores_internal_and_boundary_connections() {
        use opossum_core::{
            meter, nodes::Dummy, nodes::NodeGroup, types::api_types::ConvertToGroupRequest,
        };

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
        // Read the group's id back from backend state rather than the response body: node_a now
        // resolves *inside* the new group, so its reported parent is the group's own uuid.
        let group_id = app_state
            .document
            .lock()
            .scenery()
            .node_recursive(node_a)
            .unwrap()
            .1;
        assert_ne!(group_id, root_id, "node_a must now be inside a new group");

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "undo of the group conversion must not error"
        );

        let document = app_state.document.lock();
        assert!(
            document.scenery().node_recursive(group_id).is_err(),
            "group node must be gone after undo"
        );
        assert!(document.scenery().node_recursive(node_a).is_ok());
        assert!(document.scenery().node_recursive(node_b).is_ok());
        assert!(document.scenery().node_recursive(node_c).is_ok());

        let connections = document
            .scenery()
            .with_group_node(root_id, NodeGroup::connections)
            .unwrap();
        assert_eq!(connections.len(), 2, "both connections must be restored");
        assert!(
            connections
                .iter()
                .any(|c| c.src_id == node_a && c.target_id == node_b),
            "the formerly-internal a->b connection must be restored"
        );
        assert!(
            connections
                .iter()
                .any(|c| c.src_id == node_b && c.target_id == node_c),
            "the formerly-boundary-crossing b->c connection must be restored"
        );
    }

    /// Regression test for the desync where a `Command` failing partway through a multi-step `apply`
    /// left the live document torn (partially mutated) while the GUI - which only reacts to a
    /// *successful* response - kept showing stale state. Hand-crafts a `Batch` whose first sub-command
    /// succeeds and second is guaranteed to fail, and asserts the document is restored byte-for-byte.
    #[actix_web::test]
    async fn test_failed_undo_rolls_back_partial_mutation() {
        use crate::undo::{Command, EdgeSnapshot};
        use opossum_core::{nodes::Dummy, types::api_types::ConnectInfo};
        use uuid::Uuid;

        let app_state = Data::new(AppState::default());
        let (root_id, node_x, node_y) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();
            let node_x = scenery.add_node(Dummy::default()).unwrap();
            let node_y = scenery.add_node(Dummy::default()).unwrap();
            (root_id, node_x, node_y)
        };

        let before = app_state.document.lock().to_opm_file_string().unwrap();

        // Step 1 succeeds (connects two real, currently-unconnected nodes); step 2 is guaranteed to
        // fail (disconnecting a connection between two uuids that don't exist).
        app_state.push_undo(Command::Batch(vec![
            Command::AddEdge(EdgeSnapshot {
                group_id: root_id,
                connect_info: ConnectInfo::new(
                    node_x,
                    "output_1".to_string(),
                    node_y,
                    "input_1".to_string(),
                    0.1,
                    false,
                ),
            }),
            Command::RemoveEdge(EdgeSnapshot {
                group_id: root_id,
                connect_info: ConnectInfo::new(
                    Uuid::new_v4(),
                    "output_1".to_string(),
                    Uuid::new_v4(),
                    "input_1".to_string(),
                    0.1,
                    false,
                ),
            }),
        ]));

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(undo_document),
        )
        .await;
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert!(
            !resp.status().is_success(),
            "the crafted second step must fail"
        );

        let after = app_state.document.lock().to_opm_file_string().unwrap();
        assert_eq!(
            before, after,
            "a failed undo must leave the document exactly as it was, including undoing the first \
             sub-command's already-applied effect"
        );
    }

    /// Regression test for two bugs in undoing a "remove port map": first, `describe()` refreshed only
    /// the group's own tab, so the restored connection and the group's own exposed port (both rendered
    /// in its *parent's* tab) never reappeared; fixing that broke a second thing, since the group's own
    /// tab also needs a refresh for its `mapped_ports` state (the "mapped" symbol on the internal node's
    /// port). Asserts `/undo` reports a `GraphNeedsRefresh` for *both* the group and its parent - like
    /// `MoveNodes` already does for its two affected tabs.
    #[actix_web::test]
    async fn test_undo_remove_port_map_refreshes_group_and_parent() {
        use opossum_core::{
            meter,
            nodes::{Dummy, NodeGroup},
        };

        let app_state = Data::new(AppState::default());
        let (root_id, group_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let n1 = group.add_node(Dummy::default()).unwrap();
            group.map_input_port(n1, "input_1", "ext_in_1").unwrap();
            let group_id = scenery.add_node(group).unwrap();

            let ext_node_a = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(ext_node_a, "output_1", group_id, "ext_in_1", meter!(0.1))
                .unwrap();
            (root_id, group_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(crate::nodes::port_mappings::remove_port_map)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{group_id}/port_mappings?external_port_name=ext_in_1&port_type=Input"
            ))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(
            body.changes.iter().any(|c| matches!(
                c,
                opossum_core::types::api_types::DocumentChange::GraphNeedsRefresh { graph_id }
                    if *graph_id == root_id
            )),
            "expected a GraphNeedsRefresh targeting the parent (root) scenery, got: {:?}",
            body.changes
        );
        assert!(
            body.changes.iter().any(|c| matches!(
                c,
                opossum_core::types::api_types::DocumentChange::GraphNeedsRefresh { graph_id }
                    if *graph_id == group_id
            )),
            "expected a GraphNeedsRefresh targeting the group's own tab too (mapped_ports lives there), got: {:?}",
            body.changes
        );
    }

    /// Regression test for the crash reported after the fix above: `GraphNeedsRefresh` for a tab already
    /// re-fetches everything in it, so a `Batch` that *also* reports a more granular change for the same
    /// tab (here, `EdgeAdded` for the connection the port-map removal tore down and undo restores) makes
    /// the GUI double-apply it - for `GraphStore.edges`, a plain `Vec`, that means the same connection
    /// twice, and `EdgesComponent` keys each edge on its endpoints, so two identical entries crash
    /// Dioxus's keyed-list diffing ("keyed siblings must each have a unique key"). Asserts `/undo`'s
    /// response contains the refresh but no separately-duplicated edge/node change for the same tab.
    #[actix_web::test]
    async fn test_undo_remove_port_map_does_not_report_duplicate_tab_changes() {
        use opossum_core::{
            meter,
            nodes::{Dummy, NodeGroup},
            types::api_types::DocumentChange,
        };

        let app_state = Data::new(AppState::default());
        let (root_id, group_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let scenery = document.scenery_mut();

            let mut group = NodeGroup::new("inner group");
            let n1 = group.add_node(Dummy::default()).unwrap();
            group.map_input_port(n1, "input_1", "ext_in_1").unwrap();
            let group_id = scenery.add_node(group).unwrap();

            let ext_node_a = scenery.add_node(Dummy::default()).unwrap();
            scenery
                .connect_nodes(ext_node_a, "output_1", group_id, "ext_in_1", meter!(0.1))
                .unwrap();
            (root_id, group_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(crate::nodes::port_mappings::remove_port_map)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{group_id}/port_mappings?external_port_name=ext_in_1&port_type=Input"
            ))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;

        let refresh_count = body
            .changes
            .iter()
            .filter(|c| matches!(c, DocumentChange::GraphNeedsRefresh { graph_id } if *graph_id == root_id))
            .count();
        assert_eq!(
            refresh_count, 1,
            "expected exactly one GraphNeedsRefresh for the parent tab, got: {:?}",
            body.changes
        );
        assert!(
            !body.changes.iter().any(|c| matches!(
                c,
                DocumentChange::EdgeAdded { graph_id, .. }
                    | DocumentChange::EdgeRemoved { graph_id, .. }
                    | DocumentChange::EdgeUpdated { graph_id, .. }
                    | DocumentChange::NodeAdded { graph_id, .. }
                    | DocumentChange::NodeRemoved { graph_id, .. }
                    | DocumentChange::NodePatched { graph_id, .. }
                    if *graph_id == root_id
            )),
            "a change already covered by the GraphNeedsRefresh must not also appear separately, got: {:?}",
            body.changes
        );
    }
}
