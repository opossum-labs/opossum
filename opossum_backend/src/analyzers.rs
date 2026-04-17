use actix_web::{
    HttpRequest, HttpResponse, delete, get, patch, post, put,
    web::{self},
};
use nalgebra::Point2;
use opossum_core::{
    error::OpossumError,
    opm_document::AnalyzerInfo,
    prelude::AnalyzerType,
    types::api_types::{ErrorResponse, NewAnalyzerInfo},
};
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

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
#[post("/")]
pub async fn post_analyzer(
    data: web::Data<AppState>,
    analyzer: web::Json<NewAnalyzerInfo>,
) -> HttpResponse {
    let new_analyzer_info = analyzer.into_inner();
    let uuid = data.document.lock().add_analyzer_with_position(
        new_analyzer_info.analyzer_type,
        Some(new_analyzer_info.gui_position),
    );
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
    data.document.lock().remove_analyzer(uuid)?;
    Ok(HttpResponse::NoContent().finish())
}

/// Get a list of all analyzers of this model
#[utoipa::path(
    tag = "analyzer",
    responses((status = OK, description = "List of analyzers", body = Vec<AnalyzerInfo>)),
)]
#[get("/")]
pub async fn get_analyzers(data: web::Data<AppState>) -> HttpResponse {
    let analyzers_map = data.document.lock().analyzers();
    let analyzers: Vec<AnalyzerInfo> = analyzers_map
        .values()
        .map(|a| {
            AnalyzerInfo::new(
                a.analyzer_type().clone(),
                a.id(),
                a.gui_position().map_or(Point2::new(0.0, 0.0), |p| p),
            )
        })
        .collect();
    HttpResponse::Ok().json(analyzers)
}

/// Update the analyzer config of an analyzer node
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    request_body(content = String, description = "updated config of analyzer", content_type = "application/ron", example= "\"analyzer_type\""),
    responses(
        (status = NO_CONTENT, description = "Analyzer config successfully updated"), // <-- HIER: 204 No Content
        (status = BAD_REQUEST, body = ErrorResponse, description = "Invalid RON or UUID not found", content_type="application/json")
    )
)]
#[patch("/{uuid}")]
pub async fn patch_analyzer(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let analyzer_type: AnalyzerType = ron::de::from_str(&body).map_err(|e| {
        BackEndErrorResponse::new(
            400,
            "Parse Error",
            &format!("Failed to deserialize AnalyzerType: {e}"),
        )
    })?;
    if let Some(analyzer_info) = data.document.lock().analyzer_mut(uuid) {
        analyzer_info.set_analyzer_type(&analyzer_type);
    } else {
        return Err(BackEndErrorResponse::new(
            404,
            "Opossum",
            "UUID not found in analyzers",
        ));
    }
    Ok(HttpResponse::NoContent().finish())
}

/// Map a `SourcePort` node to a light definition in an analyzer
///
/// Adds or updates the light definition (e.g. `RayDataBuilder` or `EnergyDataBuilder`)
/// for a specific `SourcePort` node inside this analyzer's configuration.
#[utoipa::path(
    tag = "analyzer",
    params(
        ("analyzer_uuid" = Uuid, Path, description = "UUID of the analyzer"),
        ("node_uuid" = Uuid, Path, description = "UUID of the SourcePort node")
    ),
    request_body(content = String, description = "Source configuration as RON string", content_type = "application/ron"),
    responses(
        (status = NO_CONTENT, description = "Source mapping successfully created/updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Invalid RON format or UUID not found", content_type="application/json")
    )
)]
#[put("/{analyzer_uuid}/sources/{node_uuid}")]
pub async fn put_analyzer_source(
    data: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let (analyzer_uuid, node_uuid) = path.into_inner();
    let mut document = data.document.lock();

    let analyzer_info = document
        .analyzer_mut(analyzer_uuid)
        .ok_or_else(|| BackEndErrorResponse::new(404, "Opossum", "Analyzer UUID not found"))?;

    let mut a_type = analyzer_info.analyzer_type().clone();

    // Dynamisches Parsen basierend auf dem Analyzer-Typ
    match &mut a_type {
        AnalyzerType::Energy(config) => {
            let builder: opossum_core::light::lightdata::energy_data_builder::EnergyDataBuilder =
                ron::from_str(&body).map_err(|e| {
                    BackEndErrorResponse::new(
                        400,
                        "Parse Error",
                        &format!("Failed to parse EnergyDataBuilder: {e}"),
                    )
                })?;
            config.map_source(node_uuid, builder);
        }
        AnalyzerType::RayTrace(config) => {
            let builder: opossum_core::light::lightdata::ray_data_builder::RayDataBuilder =
                ron::from_str(&body).map_err(|e| {
                    BackEndErrorResponse::new(
                        400,
                        "Parse Error",
                        &format!("Failed to parse RayDataBuilder: {e}"),
                    )
                })?;
            config.map_source(node_uuid, builder);
        }
        AnalyzerType::GhostFocus(config) => {
            let builder: opossum_core::light::lightdata::ray_data_builder::RayDataBuilder =
                ron::from_str(&body).map_err(|e| {
                    BackEndErrorResponse::new(
                        400,
                        "Parse Error",
                        &format!("Failed to parse RayDataBuilder for GhostFocus: {e}"),
                    )
                })?;
            config.map_source(node_uuid, builder);
        }
    }
    analyzer_info.set_analyzer_type(&a_type);
    drop(document);
    Ok(HttpResponse::NoContent().finish())
}

/// Remove a `SourcePort` mapping from an analyzer
///
/// Removes the specific `SourcePort` light definition from this analyzer's configuration.
#[utoipa::path(
    tag = "analyzer",
    params(
        ("analyzer_uuid" = Uuid, Path, description = "UUID of the analyzer"),
        ("node_uuid" = Uuid, Path, description = "UUID of the SourcePort node to remove")
    ),
    responses(
        (status = NO_CONTENT, description = "Source mapping successfully removed"),
        (status = NOT_FOUND, body = ErrorResponse, description = "Analyzer UUID not found", content_type="application/json")
    )
)]
#[delete("/{analyzer_uuid}/sources/{node_uuid}")]
pub async fn delete_analyzer_source(
    data: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let (analyzer_uuid, node_uuid) = path.into_inner();
    let mut document = data.document.lock();

    let analyzer_info = document
        .analyzer_mut(analyzer_uuid)
        .ok_or_else(|| BackEndErrorResponse::new(404, "Opossum", "Analyzer UUID not found"))?;

    let mut a_type = analyzer_info.analyzer_type().clone();

    match &mut a_type {
        AnalyzerType::Energy(config) => {
            let _ = config.remove_source(&node_uuid);
        }
        AnalyzerType::RayTrace(config) => {
            let _ = config.remove_source(&node_uuid);
        }
        AnalyzerType::GhostFocus(config) => {
            let _ = config.remove_source(&node_uuid);
        }
    }
    analyzer_info.set_analyzer_type(&a_type);
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_analyzers);
    cfg.service(get_analyzer);
    cfg.service(post_analyzer);
    cfg.service(patch_analyzer);
    cfg.service(delete_analyzer);
    cfg.service(put_analyzer_source);
    cfg.service(delete_analyzer_source);
}
