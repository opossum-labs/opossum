use actix_web::{
    HttpRequest, HttpResponse, Responder, delete, get, patch, post,
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
///
/// # Important
/// Due to the fact that numeric properties can have values such as `NaN` or `Inf`,
/// the RON format is supported and often preferred. JSON simply returns these as `null`.
#[utoipa::path(
    tag = "analyzer",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the analyzer"),
    ),
    responses(
        (
            status = OK,
            description = "Analyzer information successfully retrieved", 
            content((AnalyzerInfo = "application/json"),( AnalyzerInfo="application/ron"))
        ),
        (
            status = BAD_REQUEST,
            body = ErrorResponse,
            description = "UUID not found", 
            content_type = "application/json"
        )
    )
)]
#[get("/{uuid}")]
async fn get_analyzer(
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
        .map_or(false, |s| s.contains("application/ron"));
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
        // Actix nimmt uns die JSON-Serialisierung automatisch ab
        Ok(HttpResponse::Ok().json(analyzer_info))
    }
}
// Helper function to contain the core logic
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
#[utoipa::path(tag = "analyzer", request_body(content = NewAnalyzerInfo,
    description = "type and GUI position of node the analyzer to be created",
    content_type = "application/json",
    example ="{\"analyzer_type\": \"Energy\", \"gui_position\": [0,0,0]}"
),
    responses((status = CREATED, body = Uuid, )))]
/// Add an analyzer to the model
///
/// This function adds an analyzer to the model.
#[post("/")]
async fn post_analyzer(
    data: web::Data<AppState>,
    analyzer: web::Json<NewAnalyzerInfo>,
) -> HttpResponse {
    let new_analyzer_info = analyzer.into_inner();
    let uuid = data.document.lock().add_analyzer_with_position(
        new_analyzer_info.analyzer_type,
        Some(new_analyzer_info.gui_position),
    );
    HttpResponse::Created().json(uuid)
}
#[utoipa::path(tag = "analyzer",
    responses((status = NO_CONTENT, description = "Analyzer deleted"),
    (status = 404, description = "Analyzer not found"))
)]
/// Delete an analyzer
///
/// This function deletes the analyzer with the given index.
#[delete("/{uuid}")]
async fn delete_analyzer(
    data: web::Data<AppState>,
    index: web::Path<Uuid>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = index.into_inner();
    data.document.lock().remove_analyzer(uuid)?;
    Ok(HttpResponse::NoContent().finish())
}
#[utoipa::path(tag = "analyzer",
    responses((status = 200, description = "List of analyzers", body = Vec<AnalyzerInfo>)),
)]
/// Get a list of all analyzers of this model
///
/// This function returns a list of all analyzers of this model. Use the index to get a specific
/// analyzer.
#[get("/")]
async fn get_analyzers(data: web::Data<AppState>) -> impl Responder {
    let analyzers = data.document.lock().analyzers();
    let analyzers: Vec<AnalyzerInfo> = analyzers
        .values()
        .map(|a| {
            AnalyzerInfo::new(
                a.analyzer_type().clone(),
                a.id(),
                a.gui_position().map_or(Point2::new(0.0, 0.0), |p| p),
            )
        })
        .collect();
    web::Json(analyzers)
}
/// Update the analyzer config of an analyzer node
#[utoipa::path(tag = "analyzer",
    params(
        ("uuid" = Uuid, Path, description = "Update an analyzer config of the analyzer node"),
    ),
    request_body(content = String,
        description = "updated config of analyzer",
        content_type = "application/ron",
        example= "\"analyzer_type\""
    ),
    responses(
        (status = OK, description = "Analyzer config successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/ron")
    )
)]
#[patch("/{uuid}")]
async fn patch_analyzer(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: String,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid: Uuid = path.into_inner();
    let analyzer_type: AnalyzerType = match ron::de::from_str(body.as_str()) {
        Ok(analyzer_type) => analyzer_type,
        Err(e) => {
            return Err(BackEndErrorResponse::new(
                400,
                "Opossum",
                &format!("Failed to deserialize property value: {e}"),
            ));
        }
    };
    let mut document = data.document.lock();
    if let Some(analyzer_info) = document.analyzer_mut(uuid) {
        analyzer_info.set_analyzer_type(&analyzer_type);
        drop(document);
    } else {
        return Err(BackEndErrorResponse::new(
            404,
            "Opossum",
            "uuid not found in analyzers",
        ));
    }
    Ok(HttpResponse::Ok()
        .content_type("application/ron")
        .body(ron::ser::to_string("").unwrap()))
}
pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_analyzers);
    cfg.service(get_analyzer);
    cfg.service(post_analyzer);
    cfg.service(patch_analyzer);
    cfg.service(delete_analyzer);
}
