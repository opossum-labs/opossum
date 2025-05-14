//! General endpoints
use std::fmt::Display;

use actix_web::{get, web::Json, Responder};
use opossum::analyzers::AnalyzerType;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;

use crate::error::ErrorResponse;

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
pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_version);
    cfg.service(get_hello);
    cfg.service(get_node_types);
    cfg.service(get_analyzer_types);
}
#[cfg(test)]
mod test {
    use super::*;
    use actix_web::{body::to_bytes, dev::Service, http::StatusCode, test, App};

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
