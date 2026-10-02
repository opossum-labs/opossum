//! Routes for managing the document
use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{analyzer_mut_or_404, parent_group_id_or_self},
    sse_logger::SENDER,
    undo::{Command, PatchNode, RepositionAnalyzer, SetViewport, capture_old_node_request},
};
use actix_web::{
    Error, HttpResponse, Responder, delete, get, patch, post, put,
    web::{self, Json},
};
use futures_util::StreamExt;
use log::{error, info, warn};
use opossum_core::{
    core_optics::node_attr::HasNodeAttr,
    opm_document::OpmDocument,
    types::api_types::{
        DocumentChange, ErrorResponse, JumpTarget, LoadDocumentResponse, PositionUpdate,
        UndoRedoResponse, UpdateNodeRequest, ViewportChangeRequest,
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
    responses((status = 200, description = "Scenery Uuid"))
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
/// the redo history. Returns the concrete changes this made plus the resulting undo/redo availability.
#[utoipa::path(tag = "document",
    responses(
        (status = OK, description = "Undo applied", body = UndoRedoResponse),
        (status = 409, description = "Nothing to undo", body = ErrorResponse)
    )
)]
#[post("/undo")]
pub async fn undo_document(
    data: web::Data<AppState>,
) -> Result<Json<UndoRedoResponse>, BackEndErrorResponse> {
    let Some(command) = data.undo_stack.lock().pop_back() else {
        return Err(BackEndErrorResponse::new(409, "Opossum", "Nothing to undo"));
    };

    match apply_history_step(&data, command.clone()) {
        Ok((changes, jump, inverse)) => {
            data.redo_stack.lock().push_back(inverse);
            Ok(Json(UndoRedoResponse {
                changes,
                jump,
                can_undo: !data.undo_stack.lock().is_empty(),
                can_redo: true,
            }))
        }
        Err(err) => {
            data.undo_stack.lock().push_back(command);
            Err(err)
        }
    }
}

/// Runs one undo or redo step: describes `command` for the GUI and computes its [`JumpTarget`], then
/// applies it under a rollback guard, returning `(changes, jump, inverse)`.
fn apply_history_step(
    data: &AppState,
    command: Command,
) -> Result<(Vec<DocumentChange>, Option<JumpTarget>, Command), BackEndErrorResponse> {
    let mut document = data.document.lock();
    let changes = command.describe(&document)?;
    let jump = command.jump_target(document.scenery().node_attr().uuid());

    let inverse = if command.needs_rollback() {
        with_rollback(&mut document, |d| command.apply(d))?
    } else {
        command.apply(&mut document)?
    };
    drop(document);
    Ok((changes, jump, inverse))
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
pub async fn redo_document(
    data: web::Data<AppState>,
) -> Result<Json<UndoRedoResponse>, BackEndErrorResponse> {
    let Some(command) = data.redo_stack.lock().pop_back() else {
        return Err(BackEndErrorResponse::new(409, "Opossum", "Nothing to redo"));
    };

    match apply_history_step(&data, command.clone()) {
        Ok((changes, jump, inverse)) => {
            data.undo_stack.lock().push_back(inverse);
            Ok(Json(UndoRedoResponse {
                changes,
                jump,
                can_undo: true,
                can_redo: !data.redo_stack.lock().is_empty(),
            }))
        }
        Err(err) => {
            data.redo_stack.lock().push_back(command);
            Err(err)
        }
    }
}

/// Record a canvas viewport change (pan/zoom of a tab) as its own undo step.
#[utoipa::path(tag = "document",
    request_body(content = ViewportChangeRequest, description = "The viewport before/after the gesture, whether it may coalesce, and whether it folds into the previous edit"),
    responses((status = NO_CONTENT, description = "Viewport change recorded"))
)]
#[post("/viewport_change")]
async fn post_viewport_change(
    data: web::Data<AppState>,
    body: web::Json<ViewportChangeRequest>,
) -> impl Responder {
    let ViewportChangeRequest {
        before,
        after,
        coalesce,
        merge_into_previous,
    } = body.into_inner();

    if before == after {
        return HttpResponse::NoContent().finish();
    }

    let before_graph_id = before.graph_id;
    let mut undo_stack = data.undo_stack.lock();
    if merge_into_previous && let Some(Command::Batch(commands)) = undo_stack.back_mut() {
        commands.push(Command::SetViewport(SetViewport {
            from: after,
            to: before,
            coalescing: coalesce,
        }));
        drop(undo_stack);
    } else if coalesce
        && let Some(Command::SetViewport(top)) = undo_stack.back_mut()
        && top.coalescing
        && top.to.graph_id == before_graph_id
    {
        top.from = after;
        drop(undo_stack);
        data.redo_stack.lock().clear();
    } else {
        drop(undo_stack);
        data.push_undo(Command::SetViewport(SetViewport {
            from: after,
            to: before,
            coalescing: coalesce,
        }));
    }
    HttpResponse::NoContent().finish()
}

/// Batch-update the GUI positions of several nodes/analyzers in one step.
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
    let mut inverses = apply_position_updates(&mut document, updates)?;
    inverses.reverse();
    data.push_undo(Command::Batch(inverses));
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

/// Applies each position update to `document` in order, returning the inverse commands.
pub fn apply_position_updates(
    document: &mut OpmDocument,
    updates: Vec<PositionUpdate>,
) -> Result<Vec<Command>, BackEndErrorResponse> {
    let mut inverses = Vec::with_capacity(updates.len());
    for update in updates {
        match apply_one_position_update(document, &update) {
            Ok(inverse) => inverses.push(inverse),
            Err(err) => {
                for inverse in inverses.into_iter().rev() {
                    let _ = inverse.apply(document);
                }
                return Err(err);
            }
        }
    }
    Ok(inverses)
}

fn apply_one_position_update(
    document: &mut OpmDocument,
    update: &PositionUpdate,
) -> Result<Command, BackEndErrorResponse> {
    if update.is_optical {
        let new = UpdateNodeRequest {
            gui_position: Some(Some(update.gui_position)),
            ..Default::default()
        };
        let old = document
            .scenery()
            .with_node_attr(update.uuid, |node_attr| {
                capture_old_node_request(node_attr, &new)
            })?;
        let parent_group_id = parent_group_id_or_self(document.scenery(), update.uuid)?;
        Command::PatchNode(Box::new(PatchNode {
            uuid: update.uuid,
            parent_group_id,
            old,
            new,
        }))
        .apply(document)
    } else {
        let old_pos = analyzer_mut_or_404(document, update.uuid)?
            .gui_position()
            .map_or((0., 0.), |p| (p.x, p.y));
        Command::RepositionAnalyzer(RepositionAnalyzer {
            id: update.uuid,
            old_pos,
            new_pos: update.gui_position,
        })
        .apply(document)
    }
}

fn with_rollback<T>(
    document: &mut OpmDocument,
    mutate: impl FnOnce(&mut OpmDocument) -> Result<T, BackEndErrorResponse>,
) -> Result<T, BackEndErrorResponse> {
    let backup = document.to_opm_file_string()?;
    match mutate(document) {
        Ok(value) => Ok(value),
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
#[post("/simulate")]
async fn simulate(data: web::Data<AppState>, report_dir: String) -> impl Responder {
    let (tx, rx) = mpsc::channel(10);
    let mut document = data.document.lock().clone();

    web::block(move || {
        SENDER.with(|cell| {
            *cell.borrow_mut() = Some(tx);
        });
        match PathBuf::from_str(&report_dir) {
            Ok(report_dir) => {
                info!("Creating report directory: {}", report_dir.display());
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
                    }
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
    .ok();

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(
            ReceiverStream::new(rx).map(|s| -> Result<actix_web::web::Bytes, Error> {
                Ok(actix_web::web::Bytes::from(format!("data: {s}\n\n")))
            }),
        )
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_document);
    cfg.service(put_document);
    cfg.service(delete_document);

    cfg.service(get_root_uuid);

    cfg.service(undo_document);
    cfg.service(redo_document);
    cfg.service(post_viewport_change);
    cfg.service(patch_positions);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{app_state::AppState, undo::Command};
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{
        meter,
        nodes::{ConnectionInfo, GraphDelta, create_node_ref},
    };

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

    /// Tests that adding a node, undoing (removing it), and redoing (restoring it)
    /// keeps the exact same UUID and emits matching DocumentChange events.
    #[actix_web::test]
    async fn test_undo_redo_restores_node_with_same_uuid() {
        let app_state = Data::new(AppState::default());

        let node_ref = create_node_ref("dummy").unwrap();
        let (node_uuid, delta) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let (uuid, delta) = document
                .scenery_mut()
                .with_group_node_mut(root_id, |g| g.add_node_ref_with_delta(node_ref))
                .unwrap()
                .unwrap();
            (uuid, delta)
        };
        app_state.push_undo(Command::UndoGraph(Box::new(delta)));
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
        assert_eq!(app_state.undo_stack.lock().len(), 1);

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(!body.can_undo);
        assert_eq!(body.changes.len(), 2);
    }

    #[actix_web::test]
    async fn test_patch_positions_rolls_back_on_partial_failure() {
        use opossum_core::{nodes::Dummy, types::api_types::PositionUpdate};
        use uuid::Uuid;

        let app_state = Data::new(AppState::default());
        let node_a = {
            let mut document = app_state.document.lock();
            document.scenery_mut().add_node(Dummy::default()).unwrap()
        };
        let original = app_state
            .document
            .lock()
            .scenery()
            .with_node_attr(node_a, |attr| attr.gui_position().map(|p| (p.x, p.y)))
            .unwrap();

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(patch_positions),
        )
        .await;

        let updates = vec![
            PositionUpdate {
                uuid: node_a,
                is_optical: true,
                gui_position: (10.0, 20.0),
            },
            PositionUpdate {
                uuid: Uuid::new_v4(),
                is_optical: true,
                gui_position: (30.0, 40.0),
            },
        ];
        let req = test::TestRequest::patch()
            .uri("/positions")
            .set_json(&updates)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert!(
            !resp.status().is_success(),
            "a batch with an unresolvable update must fail"
        );

        let after = app_state
            .document
            .lock()
            .scenery()
            .with_node_attr(node_a, |attr| attr.gui_position().map(|p| (p.x, p.y)))
            .unwrap();
        assert_eq!(
            after, original,
            "the already-applied first update must be unwound when the batch fails"
        );
        assert!(
            app_state.undo_stack.lock().is_empty(),
            "a failed position batch must push no undo entry"
        );
    }

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

    /// Regression test for rollback on multi-step undo failure using GraphDelta.
    #[actix_web::test]
    async fn test_failed_undo_rolls_back_partial_mutation() {
        use opossum_core::nodes::Dummy;
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

        // Step 1 succeeds (undoing NodesDisconnected connects the nodes); step 2 is guaranteed to
        // fail (undoing NodesConnected tries to disconnect a connection that doesn't exist).
        app_state.push_undo(Command::Batch(vec![
            Command::UndoGraph(Box::new(GraphDelta::NodesDisconnected {
                group_id: root_id,
                connection: ConnectionInfo {
                    src_id: node_x,
                    src_port: "output_1".to_string(),
                    target_id: node_y,
                    target_port: "input_1".to_string(),
                    distance: meter!(0.1),
                },
            })),
            Command::UndoGraph(Box::new(GraphDelta::NodesConnected {
                group_id: root_id,
                connection: ConnectionInfo {
                    src_id: Uuid::new_v4(),
                    src_port: "output_1".to_string(),
                    target_id: Uuid::new_v4(),
                    target_port: "input_1".to_string(),
                    distance: meter!(0.1),
                },
                displaced_port_mappings: Vec::new(),
            })),
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
        assert_eq!(
            app_state.undo_stack.lock().len(),
            1,
            "a failed undo must keep its command on the undo stack, not silently drop it"
        );
        assert!(
            app_state.redo_stack.lock().is_empty(),
            "a failed undo must not push anything onto the redo stack"
        );
    }

    /// Companion to `test_failed_undo_rolls_back_partial_mutation` for the redo path:
    /// a redo whose command fails partway must roll the document back and keep its entry on the redo stack.
    #[actix_web::test]
    async fn test_failed_redo_restores_command_to_redo_stack() {
        use opossum_core::nodes::Dummy;
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

        // Step 1 succeeds (NodesConnected connects node_x and node_y); step 2 fails
        // (NodesDisconnected tries to disconnect a nonexistent node).
        app_state.redo_stack.lock().push_back(Command::Batch(vec![
            Command::RedoGraph(Box::new(GraphDelta::NodesConnected {
                group_id: root_id,
                connection: ConnectionInfo {
                    src_id: node_x,
                    src_port: "output_1".to_string(),
                    target_id: node_y,
                    target_port: "input_1".to_string(),
                    distance: meter!(0.1),
                },
                displaced_port_mappings: Vec::new(),
            })),
            Command::RedoGraph(Box::new(GraphDelta::NodesDisconnected {
                group_id: root_id,
                connection: ConnectionInfo {
                    src_id: Uuid::new_v4(),
                    src_port: "output_1".to_string(),
                    target_id: Uuid::new_v4(),
                    target_port: "input_1".to_string(),
                    distance: meter!(0.1),
                },
            })),
        ]));

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(redo_document),
        )
        .await;
        let req = test::TestRequest::post().uri("/redo").to_request();
        let resp = app.call(req).await.unwrap();
        assert!(
            !resp.status().is_success(),
            "the crafted second step must fail"
        );

        let after = app_state.document.lock().to_opm_file_string().unwrap();
        assert_eq!(
            before, after,
            "a failed redo must leave the document exactly as it was"
        );
        assert_eq!(
            app_state.redo_stack.lock().len(),
            1,
            "a failed redo must keep its command on the redo stack, not silently drop it"
        );
        assert!(
            app_state.undo_stack.lock().is_empty(),
            "a failed redo must not push anything onto the undo stack"
        );
    }

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
                opossum_core::types::api_types::DocumentChange::GraphNeedsRefresh { graph_id, .. }
                    if *graph_id == root_id
            )),
            "expected a GraphNeedsRefresh targeting the parent (root) scenery, got: {:?}",
            body.changes
        );
        assert!(
            body.changes.iter().any(|c| matches!(
                c,
                opossum_core::types::api_types::DocumentChange::GraphNeedsRefresh { graph_id, .. }
                    if *graph_id == group_id
            )),
            "expected a GraphNeedsRefresh targeting the group's own tab too (mapped_ports lives there), got: {:?}",
            body.changes
        );
    }

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
            .filter(|c| matches!(c, DocumentChange::GraphNeedsRefresh { graph_id, .. } if *graph_id == root_id))
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

    #[actix_web::test]
    async fn test_viewport_change_undo_redo_round_trip() {
        use opossum_core::types::api_types::{DocumentChange, Viewport, ViewportChangeRequest};

        let app_state = Data::new(AppState::default());
        let graph_id = app_state.document.lock().scenery().node_attr().uuid();

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_viewport_change)
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        let before = Viewport {
            graph_id,
            zoom: 1.0,
            shift: (0.0, 0.0),
        };
        let after = Viewport {
            graph_id,
            zoom: 2.0,
            shift: (50.0, -10.0),
        };

        let req = test::TestRequest::post()
            .uri("/viewport_change")
            .set_json(&ViewportChangeRequest {
                before,
                after,
                coalesce: false,
                merge_into_previous: false,
            })
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
                body.changes.as_slice(),
                [DocumentChange::ViewportChanged { graph_id: g, zoom, shift }]
                    if *g == graph_id && (*zoom - 1.0).abs() < f64::EPSILON && *shift == (0.0, 0.0)
            ),
            "undo must move the camera back to `before`, got {:?}",
            body.changes
        );

        let req = test::TestRequest::post().uri("/redo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(
            matches!(
                body.changes.as_slice(),
                [DocumentChange::ViewportChanged { graph_id: g, zoom, shift }]
                    if *g == graph_id && (*zoom - 2.0).abs() < f64::EPSILON && *shift == (50.0, -10.0)
            ),
            "redo must move the camera forward to `after`, got {:?}",
            body.changes
        );
    }

    #[actix_web::test]
    async fn test_viewport_change_coalesces_consecutive_camera_moves() {
        use opossum_core::types::api_types::{DocumentChange, Viewport, ViewportChangeRequest};

        let app_state = Data::new(AppState::default());
        let graph_id = app_state.document.lock().scenery().node_attr().uuid();
        let vp = |zoom: f64| Viewport {
            graph_id,
            zoom,
            shift: (0.0, 0.0),
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_viewport_change)
                .service(undo_document),
        )
        .await;

        for (before, after) in [(vp(1.0), vp(1.5)), (vp(1.5), vp(2.0)), (vp(2.0), vp(2.5))] {
            let req = test::TestRequest::post()
                .uri("/viewport_change")
                .set_json(&ViewportChangeRequest {
                    before,
                    after,
                    coalesce: true,
                    merge_into_previous: false,
                })
                .to_request();
            assert_eq!(
                app.call(req).await.unwrap().status(),
                StatusCode::NO_CONTENT
            );
        }
        assert_eq!(
            app_state.undo_stack.lock().len(),
            1,
            "the whole burst must be a single undo step, not one per tick"
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(
            matches!(
                body.changes.as_slice(),
                [DocumentChange::ViewportChanged { zoom, .. }] if (*zoom - 1.0).abs() < f64::EPSILON
            ),
            "one undo must return to the pre-burst viewport, got {:?}",
            body.changes
        );
        assert!(
            !body.can_undo,
            "the burst was a single step, so nothing left to undo"
        );
    }

    #[actix_web::test]
    async fn test_viewport_change_does_not_coalesce_across_gesture_types() {
        use opossum_core::types::api_types::{Viewport, ViewportChangeRequest};

        let app_state = Data::new(AppState::default());
        let graph_id = app_state.document.lock().scenery().node_attr().uuid();
        let vp = |zoom: f64, x: f64| Viewport {
            graph_id,
            zoom,
            shift: (x, 0.0),
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_viewport_change),
        )
        .await;

        for (before, after, coalesce) in [
            (vp(1.0, 0.0), vp(2.0, 0.0), true),
            (vp(2.0, 0.0), vp(2.0, 100.0), false),
            (vp(2.0, 100.0), vp(2.0, 200.0), false),
        ] {
            let req = test::TestRequest::post()
                .uri("/viewport_change")
                .set_json(&ViewportChangeRequest {
                    before,
                    after,
                    coalesce,
                    merge_into_previous: false,
                })
                .to_request();
            assert_eq!(
                app.call(req).await.unwrap().status(),
                StatusCode::NO_CONTENT
            );
        }
        assert_eq!(
            app_state.undo_stack.lock().len(),
            3,
            "different gesture types (zoom, pan, pan) must each be their own undo step"
        );
    }

    #[actix_web::test]
    async fn test_viewport_change_merges_into_preceding_position_batch() {
        use opossum_core::{
            nodes::Dummy,
            types::api_types::{DocumentChange, PositionUpdate, Viewport, ViewportChangeRequest},
        };

        let app_state = Data::new(AppState::default());
        let (graph_id, node_id) = {
            let mut document = app_state.document.lock();
            let graph_id = document.scenery().node_attr().uuid();
            let node_id = document.scenery_mut().add_node(Dummy::default()).unwrap();
            (graph_id, node_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(patch_positions)
                .service(post_viewport_change)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::patch()
            .uri("/positions")
            .set_json(vec![PositionUpdate {
                uuid: node_id,
                is_optical: true,
                gui_position: (123.0, 456.0),
            }])
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let req = test::TestRequest::post()
            .uri("/viewport_change")
            .set_json(&ViewportChangeRequest {
                before: Viewport {
                    graph_id,
                    zoom: 1.0,
                    shift: (0.0, 0.0),
                },
                after: Viewport {
                    graph_id,
                    zoom: 2.0,
                    shift: (10.0, 20.0),
                },
                coalesce: false,
                merge_into_previous: true,
            })
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        assert_eq!(
            app_state.undo_stack.lock().len(),
            1,
            "the fit must fold into the position batch, not become a second undo step"
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(
            body.changes
                .iter()
                .any(|c| matches!(c, DocumentChange::ViewportChanged { .. })),
            "undo must revert the fit, got {:?}",
            body.changes
        );
        assert!(
            body.changes
                .iter()
                .any(|c| matches!(c, DocumentChange::NodePatched { .. })),
            "undo must revert the reposition, got {:?}",
            body.changes
        );
        assert!(
            !body.can_undo,
            "both were one step, so nothing left to undo"
        );
    }

    #[actix_web::test]
    async fn test_get_root_uuid() {
        let app_state = Data::new(AppState::default());
        let expected_uuid = app_state.document.lock().scenery().node_attr().uuid();

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(get_root_uuid),
        )
        .await;

        let req = test::TestRequest::get().uri("/root_uuid").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let root_uuid: uuid::Uuid = test::read_body_json(resp).await;
        assert_eq!(root_uuid, expected_uuid);
    }

    #[actix_web::test]
    async fn test_get_document_returns_ron_format() {
        let app_state = Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(web::scope("/document").service(get_document)),
        )
        .await;

        let req = test::TestRequest::get().uri("/document").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(actix_web::http::header::CONTENT_TYPE)
                .unwrap(),
            RON_MEDIA_TYPE
        );

        let body = test::read_body(resp).await;
        let body_str = std::str::from_utf8(&body).unwrap();
        assert!(OpmDocument::from_string(body_str).is_ok());
    }

    #[actix_web::test]
    async fn test_put_document_valid_and_invalid() {
        let app_state = Data::new(AppState::default());
        let valid_opm = app_state.document.lock().to_opm_file_string().unwrap();

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(web::scope("/document").service(put_document)),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/document")
            .insert_header((actix_web::http::header::CONTENT_TYPE, "text/plain"))
            .set_payload(valid_opm)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: LoadDocumentResponse = test::read_body_json(resp).await;
        assert_eq!(body.name, "group");

        let req_invalid = test::TestRequest::put()
            .uri("/document")
            .insert_header((actix_web::http::header::CONTENT_TYPE, "text/plain"))
            .set_payload("invalid ron string content")
            .to_request();
        let resp_invalid = app.call(req_invalid).await.unwrap();
        assert_eq!(resp_invalid.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_delete_document_resets_document_and_undo() {
        let app_state = Data::new(AppState::default());
        app_state
            .undo_stack
            .lock()
            .push_back(Command::Batch(vec![]));
        assert!(!app_state.undo_stack.lock().is_empty());

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(web::scope("/document").service(delete_document)),
        )
        .await;

        let req = test::TestRequest::delete().uri("/document").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        assert!(app_state.undo_stack.lock().is_empty());
        assert_eq!(
            app_state.document.lock().scenery().node_attr().name(),
            "group"
        );
    }

    #[actix_web::test]
    async fn test_viewport_change_no_op_when_identical() {
        use opossum_core::types::api_types::{Viewport, ViewportChangeRequest};

        let app_state = Data::new(AppState::default());
        let graph_id = app_state.document.lock().scenery().node_attr().uuid();
        let vp = Viewport {
            graph_id,
            zoom: 1.0,
            shift: (0.0, 0.0),
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(post_viewport_change),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/viewport_change")
            .set_json(&ViewportChangeRequest {
                before: vp.clone(),
                after: vp,
                coalesce: false,
                merge_into_previous: false,
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(app_state.undo_stack.lock().is_empty());
    }
}
