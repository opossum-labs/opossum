use actix_web::{
    HttpRequest, HttpResponse, delete, get, patch, post, put,
    web::{self},
};
use opossum_core::{
    core_optics::node_attr::HasNodeAttr,
    error::OpossumError,
    light::lightdata::{energy_data_builder::EnergyDataBuilder, ray_data_builder::RayDataBuilder},
    nodes::NodeGroup,
    opm_document::AnalyzerInfo,
    prelude::AnalyzerType,
    types::api_types::{AnalyzerItemDto, ErrorResponse, NewAnalyzerInfo, SourcePortDto},
    utils::LockExt,
};
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::Ron,
    undo::{Command, PatchAnalyzer, RepositionAnalyzer},
};

// --- NEW INTERNAL HELPER FUNCTION FOR RECURSIVE SOURCE PORT LOOKUP ---
fn get_all_source_port_uuids(scenery: &NodeGroup) -> Vec<Uuid> {
    fn walk(scenery: &NodeGroup, current_group: Uuid, collected: &mut Vec<Uuid>) {
        let children_res = scenery.with_group_node(current_group, |g| {
            g.nodes()
                .iter()
                .map(|n| {
                    let node = n.optical_ref.lock_opm()?;
                    let node_type = node.node_attr().node_type().to_string();
                    let node_uuid = node.node_attr().uuid();
                    drop(node);
                    Ok((node_uuid, node_type))
                })
                .collect::<Result<Vec<(Uuid, String)>, OpossumError>>()
        });

        if let Ok(Ok(nodes_data)) = children_res {
            for (child_uuid, node_type) in nodes_data {
                if node_type == "source port" {
                    collected.push(child_uuid);
                } else if scenery.with_group_node(child_uuid, |_| {}).is_ok() {
                    walk(scenery, child_uuid, collected);
                }
            }
        }
    }

    let mut collected = Vec::new();
    let root_uuid = scenery.node_attr().uuid();
    walk(scenery, root_uuid, &mut collected);
    collected
}

/// Get an analyzer by UUID
///
/// Returns all information (`AnalyzerInfo`) of the analyzer specified by its UUID.
/// The format is determined by the `Accept` header (`application/ron` or `application/json`).
/// Defaults to JSON.
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    responses(
        (status = OK, description = "Analyzer information successfully retrieved", content((AnalyzerInfo = "application/json"),( AnalyzerInfo="application/ron"))),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[get("/{uuid}")]
#[allow(clippy::future_not_send)]
pub async fn get_analyzer(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let analyzer_info = get_node_analyzer_attr_from_state(uuid, &data)?;
    let wants_ron = req
        .headers()
        .get(actix_web::http::header::ACCEPT)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|s| s.contains("application/ron"));

    if wants_ron {
        let body = ron::ser::to_string_pretty(
            &analyzer_info,
            ron::ser::PrettyConfig::new().new_line("\n"),
        )
        .map_err(|e| OpossumError::Other(format!("RON Serialization Error: {e}")))?;

        Ok(HttpResponse::Ok()
            .content_type("application/ron")
            .body(body))
    } else {
        Ok(HttpResponse::Ok().json(analyzer_info))
    }
}

fn get_node_analyzer_attr_from_state(
    uuid: Uuid,
    data: &web::Data<AppState>,
) -> Result<AnalyzerInfo, BackEndErrorResponse> {
    let document = data.document.lock().clone();
    let analyzer_info = document
        .analyzers()
        .get(&uuid)
        .ok_or_else(|| BackEndErrorResponse::new(404, "Opossum", "UUID not found in analyzers"))?
        .clone();
    Ok(analyzer_info)
}

/// Add an analyzer to the model
#[utoipa::path(
    tag = "analyzer", 
    request_body(content = NewAnalyzerInfo, description = "type and GUI position of the analyzer to be created", content_type = "application/json", example ="{\"analyzer_type\": \"Energy\", \"gui_position\": [0,0,0]}"),
    responses((status = CREATED, body = Uuid, description = "Analyzer successfully created"))
)]
#[post("")]
pub async fn post_analyzer(
    data: web::Data<AppState>,
    analyzer: web::Json<NewAnalyzerInfo>,
) -> HttpResponse {
    let new_analyzer_info = analyzer.into_inner();
    let mut document = data.document.lock();

    // 1. Create the analyzer core instance
    let uuid = document.add_analyzer_with_position(
        new_analyzer_info.analyzer_type,
        Some(new_analyzer_info.gui_position),
    );

    // 2. Automatically populate default mappings for all currently existing source ports
    let source_uuids = get_all_source_port_uuids(document.scenery());
    if let Some(analyzer_info) = document.analyzer_mut(uuid) {
        let mut a_type = analyzer_info.analyzer_type().clone();

        for port_uuid in source_uuids {
            match &mut a_type {
                AnalyzerType::Energy(config) => {
                    config.map_source(port_uuid, EnergyDataBuilder::default());
                }
                AnalyzerType::RayTrace(config) => {
                    config.map_source(port_uuid, RayDataBuilder::default());
                }
                AnalyzerType::GhostFocus(config) => {
                    config.map_source(port_uuid, RayDataBuilder::default());
                }
            }
        }
        analyzer_info.set_analyzer_type(&a_type);
    }

    if let Ok(info) = document.analyzer(uuid) {
        data.push_undo(Command::RemoveAnalyzer { id: uuid, info });
    }
    drop(document);
    HttpResponse::Created().json(uuid) // 201 Created
}

/// Delete an analyzer
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer to delete")),
    responses(
        (status = NO_CONTENT, description = "Analyzer deleted"),
        (status = NOT_FOUND, body = ErrorResponse, description = "Analyzer not found")
    )
)]
#[delete("/{uuid}")]
pub async fn delete_analyzer(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock();
    let info = document.analyzer(uuid)?;
    document.remove_analyzer(uuid)?;
    data.push_undo(Command::AddAnalyzer { id: uuid, info });
    drop(document);
    Ok(HttpResponse::NoContent().finish())
}

/// Get a list of all analyzers of this model
#[utoipa::path(
    tag = "analyzer",
    responses((status = OK, description = "List of analyzers", body = Vec<AnalyzerItemDto>)),
)]
#[get("")]
pub async fn get_analyzers(data: web::Data<AppState>) -> HttpResponse {
    let analyzers_map = data.document.lock().analyzers();

    // Transform the HashMap into a list of DTOs containing the Uuid and the AnalyzerInfo
    let analyzers: Vec<AnalyzerItemDto> = analyzers_map
        .into_iter()
        .map(|(id, info)| {
            AnalyzerItemDto {
                id,
                info, // info already contains the correct gui_position logic internally
            }
        })
        .collect();

    HttpResponse::Ok().json(analyzers)
}

/// Update the analyzer config of an analyzer node
///
/// *Note*: This only updates the analyzer config. A GUI position is unchanged
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    request_body(content = AnalyzerType, description = "updated config of analyzer", content_type = "application/ron", example= "\"analyzer_type\""),
    responses(
        (status = NO_CONTENT, description = "Analyzer config successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Invalid RON or UUID not found", content_type="application/json")
    )
)]
#[patch("/{uuid}")]
pub async fn patch_analyzer(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: Ron<AnalyzerType>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let new = body.into_inner();
    let mut document = data.document.lock();

    let old = document
        .analyzer_mut(uuid)
        .ok_or_else(|| BackEndErrorResponse::new(404, "Opossum", "UUID not found in analyzers"))?
        .analyzer_type()
        .clone();

    let inverse =
        Command::PatchAnalyzer(PatchAnalyzer { id: uuid, old, new }).apply(&mut document)?;
    data.push_undo(inverse);
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

/// Update the GUI position of an analyzer
///
/// This endpoint is used by the frontend to update the 2D canvas coordinates
/// of an analyzer node after a drag-and-drop operation.
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    request_body(
        content = inline((f64, f64)),
        description = "New X and Y coordinates as a simple JSON array", 
        content_type = "application/json", 
        example = json!([150.5, -20.0])
    ),
    responses(
        (status = NO_CONTENT, description = "GUI position successfully updated"),
        (status = NOT_FOUND, body = ErrorResponse, description = "Analyzer UUID not found", content_type="application/json")
    )
)]
#[put("/{uuid}/gui_position")]
pub async fn put_analyzer_gui_position(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    gui_position: web::Json<(f64, f64)>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let new_pos = gui_position.into_inner();
    let mut document = data.document.lock();

    let old_pos = document
        .analyzer_mut(uuid)
        .ok_or_else(|| BackEndErrorResponse::new(404, "Opossum", "UUID not found in analyzers"))?
        .gui_position()
        .map_or((0., 0.), |p| (p.x, p.y));

    let inverse = Command::RepositionAnalyzer(RepositionAnalyzer {
        id: uuid,
        old_pos,
        new_pos,
    })
    .apply(&mut document)?;
    data.push_undo(inverse);
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

/// Get all available `SourcePort` nodes (UUID and Name) in the entire document recursively
#[utoipa::path(
    tag = "analyzer",
    responses(
        (status = OK, description = "List of all available SourcePort pairs in the document", body = Vec<SourcePortDto>),
        (status = INTERNAL_SERVER_ERROR, body = ErrorResponse, description = "Internal tree traversal error")
    )
)]
#[get("/available_sources")]
pub async fn get_available_sources(
    data: web::Data<AppState>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    // Local helper function for recursive tree walk
    fn walk_scenery(
        scenery: &NodeGroup,
        current_group_uuid: Uuid,
        collected: &mut Vec<SourcePortDto>,
    ) -> Result<(), OpossumError> {
        let children_res = scenery.with_group_node(current_group_uuid, |g| {
            g.nodes()
                .iter()
                .map(|n| {
                    let node = n.optical_ref.lock_opm()?;
                    let node_type = node.node_attr().node_type().to_string();
                    let node_uuid = node.node_attr().uuid();
                    let node_name = node.node_attr().name().to_string();
                    drop(node);
                    Ok((node_uuid, node_type, node_name))
                })
                .collect::<Result<Vec<(Uuid, String, String)>, OpossumError>>()
        });

        if let Ok(Ok(nodes_data)) = children_res {
            for (child_uuid, node_type, node_name) in nodes_data {
                if node_type == "source port" {
                    // Collect only the minimal required information
                    collected.push(SourcePortDto {
                        uuid: child_uuid,
                        name: node_name,
                    });
                } else {
                    // Recurse deeper if the node acts as a sub-group
                    if scenery.with_group_node(child_uuid, |_| {}).is_ok() {
                        walk_scenery(scenery, child_uuid, collected)?;
                    }
                }
            }
        }
        Ok(())
    }

    let document = data.document.lock();
    let scenery = document.scenery().clone();
    let root_uuid = scenery.node_attr().uuid();
    drop(document);
    let mut collected_sources = Vec::new();

    walk_scenery(&scenery, root_uuid, &mut collected_sources)
        .map_err(|e| BackEndErrorResponse::new(500, "OpticScenery", &e.to_string()))?;

    Ok(HttpResponse::Ok().json(collected_sources))
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_available_sources);
    cfg.service(get_analyzers);
    cfg.service(get_analyzer);
    cfg.service(post_analyzer);
    cfg.service(patch_analyzer);
    cfg.service(delete_analyzer);
    cfg.service(put_analyzer_gui_position);
}
