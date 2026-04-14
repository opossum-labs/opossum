//! General endpoints

use crate::{app_state::AppState, error::BackEndErrorResponse};
use actix_web::{
    HttpResponse, Responder, get, post,
    web::{self, Json},
};
use opossum_core::{
    analyzers::AnalyzerType,
    reporting::analysis_report::AnalysisReport,
    types::api_types::{NodeType, VersionInfo},
};
use semver::Version;
use serde::Deserialize;
use utoipa_actix_web::service_config::ServiceConfig;

/// Return a welcome message
///
/// Simply return the text `OPOSSUM backend`. This is mostly for checking that the client is communication with the correct server.
#[utoipa::path(get, path="/", responses((status = OK, description = "Fixed answer string", body = str, example = "OPOSSUM backend")), tag="general")]
#[get("/")]
async fn get_hello() -> &'static str {
    "OPOSSUM backend"
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

/// Return a version information
///
/// Return the version numbers of the OPOSSUM library and the backend server including a check for updates on GitHub.
#[utoipa::path(get, responses((status = OK, description = "success", body = VersionInfo)), tag="general")]
#[get("/version")]
async fn get_version() -> impl Responder {
    let backend_version = env!("CARGO_PKG_VERSION").to_string();
    let opossum_version = opossum_core::get_version();

    let mut latest_github_version = None;
    let mut release_url = None;
    let mut update_available = false;

    // Try to call GitHub API
    let client = reqwest::Client::new();
    let res = client
        .get("https://api.github.com/repos/opossum-labs/opossum/releases/latest")
        .header("User-Agent", "OPOSSUM-Backend")
        .send()
        .await;

    // If successful, parse the information
    if let Ok(response) = res
        && response.status().is_success()
        && let Ok(release) = response.json::<GitHubRelease>().await
    {
        latest_github_version = Some(release.tag_name.clone());
        release_url = Some(release.html_url);

        // Compare versions with semver
        if let (Ok(local_ver), Ok(github_ver)) = (
            Version::parse(&backend_version),
            Version::parse(&release.tag_name),
        ) {
            update_available = github_ver > local_ver;
        }
    }

    Json(VersionInfo {
        backend_version,
        opossum_version,
        latest_github_version,
        release_url,
        update_available,
    })
}

/// Return a list of all available optical node types
///
/// Return an alphabetically sorted list of strings of all available node types present in the OPOSSUM library.
#[utoipa::path(get, responses((status = OK, description = "success", body = Vec<NodeType>)), tag="general")]
#[get("/node_types")]
async fn get_node_types() -> Result<Json<Vec<NodeType>>, BackEndErrorResponse> {
    let types = opossum_core::nodes::node_types();
    let mut node_types: Vec<NodeType> = types
        .iter()
        .map(|t| NodeType {
            node_type: t.0.into(),
            description: t.1.into(),
        })
        .collect();
    node_types.sort_by(|a, b| a.node_type.to_lowercase().cmp(&b.node_type.to_lowercase()));
    Ok(Json(node_types))
}
/// Return a list of available analyzer types of OPOSSUM
///
/// Return a list of all available analyzer types from the OPOSSUM library.
#[utoipa::path(get, responses((status = OK, description = "success", body = Vec<AnalyzerType>)), tag="general")]
#[get("/analyzer_types")]
async fn get_analyzer_types() -> Result<Json<Vec<AnalyzerType>>, BackEndErrorResponse> {
    let analyzer_types = opossum_core::analyzers::AnalyzerType::analyzer_types();
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
    let server_handle = data.server_handle.lock().clone();
    server_handle.unwrap().stop(true).await;
    HttpResponse::NoContent().finish()
}

/// Analyze current setup and eturn a vector of analysisreports
#[utoipa::path(get, responses(
    (status = OK, description = "success", content_type="application/json"),
    (status = BAD_REQUEST, body = BackEndErrorResponse, description = "Error during analysis", content_type="application/json")

), tag="general")]
#[get("/analyze")]
async fn get_analyze(
    data: web::Data<AppState>,
) -> Result<Json<Vec<AnalysisReport>>, BackEndErrorResponse> {
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
