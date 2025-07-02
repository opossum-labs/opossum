//! General endpoints
use std::fmt::Display;

use actix_web::{
    HttpResponse, Responder, get, post,
    web::{self, Json},
};
use opossum::{analyzers::AnalyzerType, reporting::analysis_report::AnalysisReport};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;

use crate::{app_state::AppState, error::ErrorResponse};

/// Structure holding the version information
#[derive(ToSchema, Serialize, Deserialize)]
pub struct VersionInfo {
    /// version of the OPOSSUM API backend
    #[schema(example = "0.1.0")]
    backend_version: String,
    /// version of the OPOSSUM library (possibly including the git hash)
    #[schema(example = "0.6.0-18-g80cb67f (2025/02/19 15:29)")]
    opossum_version: String,
}

impl VersionInfo {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn backend_version(&self) -> &str {
        &self.backend_version
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn opossum_version(&self) -> &str {
        &self.opossum_version
    }
}

/// Return a welcome message
///
/// Simply return the text `OPOSSUM backend`. This is mostly for checking that the client is communication with the correct server.
#[utoipa::path(get, path="/", responses((status = OK, description = "Fixed answer string", body = str, example = "OPOSSUM backend")), tag="general")]
#[get("/")]
async fn get_hello() -> &'static str {
    "OPOSSUM backend"
}

/// Return a version information
///
/// Return the version numbers of the OPOSSUM library and the backend server.
#[utoipa::path(get, responses((status = OK, description = "success", body = VersionInfo)), tag="general")]
#[get("/version")]
async fn get_version() -> impl Responder {
    Json(VersionInfo {
        backend_version: env!("CARGO_PKG_VERSION").to_string(),
        opossum_version: opossum::get_version(),
    })
}
#[derive(Deserialize, Serialize, ToSchema)]
pub struct NodeType {
    node_type: String,
    description: String,
}
impl Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.node_type)
    }
}
/// Return a list of all available node types of OPOSSUM
///
/// Return a list of strings of available node types from the OPOSSUM library.
#[utoipa::path(get, responses((status = OK, description = "success", body = Vec<NodeType>)), tag="general")]
#[get("/node_types")]
async fn get_node_types() -> Result<Json<Vec<NodeType>>, ErrorResponse> {
    let types = opossum::nodes::node_types();
    let node_types: Vec<NodeType> = types
        .iter()
        .map(|t| NodeType {
            node_type: t.0.into(),
            description: t.1.into(),
        })
        .collect();
    Ok(Json(node_types))
}
/// Return a list of available analyzer types of OPOSSUM
///
/// Return a list of all available analyzer types from the OPOSSUM library.
#[utoipa::path(get, responses((status = OK, description = "success", body = Vec<AnalyzerType>)), tag="general")]
#[get("/analyzer_types")]
async fn get_analyzer_types() -> Result<Json<Vec<AnalyzerType>>, ErrorResponse> {
    let analyzer_types = opossum::analyzers::AnalyzerType::analyzer_types();
    Ok(Json(analyzer_types))
}
/// Terminate the backend server
///
/// This terminates the OPOSSUM backend server. This is a (probably temporary) endpoint which is used to kill the server
/// when the GUI is closed. It might be removed in the future. **Note**: After sending this call you can no longer communicate as
/// the server is closed.
#[utoipa::path(post, responses((status = 204, description = "success")), tag="general")]
#[post("/terminate")]
async fn post_terminate(data: web::Data<AppState>) -> HttpResponse {
    data.server_handle.lock().as_ref().unwrap().stop(true).await;
    HttpResponse::NoContent().finish()
}

/// Analyze current setup and eturn a vector of analysisreports
#[utoipa::path(get, responses(
    (status = OK, description = "success", content_type="application/json"),
    (status = BAD_REQUEST, body = ErrorResponse, description = "Error during analysis", content_type="application/json")

), tag="general")]
#[get("/analyze")]
async fn get_analyze(
    data: web::Data<AppState>,
) -> Result<Json<Vec<AnalysisReport>>, ErrorResponse> {
    let mut document = data.document.lock();
    let reports = document.analyze()?;
    drop(document);
    Ok(Json(reports))
}
pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_version);
    cfg.service(get_hello);
    cfg.service(get_node_types);
    cfg.service(get_analyzer_types);
    cfg.service(post_terminate);
    cfg.service(get_analyze);
}
#[cfg(test)]
mod test {
    use super::*;
    use actix_web::{App, body::to_bytes, dev::Service, http::StatusCode, test};

    #[actix_web::test]
    async fn get_hello() {
        let app = test::init_service(App::new().service(super::get_hello)).await;
        let req = test::TestRequest::get().uri("/").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response_body = resp.into_body();
        assert_eq!(to_bytes(response_body).await.unwrap(), "OPOSSUM backend");
    }
    #[actix_web::test]
    async fn get_version() {
        let app = test::init_service(App::new().service(super::get_version)).await;
        let req = test::TestRequest::get().uri("/version").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let _: VersionInfo = test::read_body_json(resp).await;
    }
    #[actix_web::test]
    async fn get_node_types() {
        let app = test::init_service(App::new().service(super::get_node_types)).await;
        let req = test::TestRequest::get().uri("/node_types").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let _: Vec<NodeType> = test::read_body_json(resp).await;
    }
    #[actix_web::test]
    async fn get_analyzer_types() {
        let app = test::init_service(App::new().service(super::get_analyzer_types)).await;
        let req = test::TestRequest::get().uri("/analyzer_types").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let _: Vec<AnalyzerType> = test::read_body_json(resp).await;
    }
}
