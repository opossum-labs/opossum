//! General endpoints
use actix_web::{
    get,
    web::{self, Data},
    HttpResponse, Responder, Result,
};
use opossum::get_version;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;

use crate::app_state::AppState;

/// Generate a nice welcome
///
/// Simply return the text `OPOSSUM backend`.
#[utoipa::path(get, path="/", responses((status = 200, body = str)), tag="general")]
#[get("/")]
pub async fn hello() -> impl Responder {
    HttpResponse::Ok().body("OPOSSUM backend")
}

/// Structure holding the version information
#[derive(ToSchema, Serialize)]
struct VersionInfo {
    /// version of the OPOSSUM API backend
    #[schema(example = "0.1.0")]
    backend_version: String,
    /// version of the OPOSSUM library (possibly including the git hash)
    #[schema(example = "0.6.0-18-g80cb67f (2025/02/19 15:29)")]
    opossum_version: String,
}

#[utoipa::path(get, responses((status = OK, body = VersionInfo)), tag="general")]
#[get("/version")]
pub async fn version() -> Result<impl Responder> {
    let version_info = VersionInfo {
        backend_version: env!("CARGO_PKG_VERSION").to_string(),
        opossum_version: get_version(),
    };
    Ok(web::Json(version_info))
}

pub fn configure(store: Data<AppState>) -> impl FnOnce(&mut ServiceConfig<'_>) {
    |config: &mut ServiceConfig<'_>| {
        config.app_data(store).service(version).service(hello);
    }
}
