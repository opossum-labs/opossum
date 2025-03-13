//! General endpoints
use crate::app_state::AppState;
use actix_web::{
    get,
    web::{self, Data},
    Responder,
};
use opossum::get_version;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;

/// Generate a nice welcome
///
/// Simply return the text `OPOSSUM backend`.
#[utoipa::path(get, path="/", responses((status = 200, body = str)), tag="general")]
#[get("/")]
pub async fn hello() -> impl Responder {
    "OPOSSUM backend"
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
pub async fn version() -> impl Responder {
    web::Json(VersionInfo {
        backend_version: env!("CARGO_PKG_VERSION").to_string(),
        opossum_version: get_version(),
    })
}

pub fn configure(store: Data<AppState>) -> impl FnOnce(&mut ServiceConfig<'_>) {
    |config: &mut ServiceConfig<'_>| {
        config.app_data(store).service(version).service(hello);
    }
}
