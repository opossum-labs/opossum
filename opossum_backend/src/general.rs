//! General endpoints
use crate::app_state::AppState;
use actix_web::{
    get,
    web::{self, Data},
    Responder,
};
use opossum::get_version;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;

/// Structure holding the version information
#[derive(ToSchema, Serialize, Deserialize)]
struct VersionInfo {
    /// version of the OPOSSUM API backend
    #[schema(example = "0.1.0")]
    backend_version: String,
    /// version of the OPOSSUM library (possibly including the git hash)
    #[schema(example = "0.6.0-18-g80cb67f (2025/02/19 15:29)")]
    opossum_version: String,
}

/// Return a welcome message
///
/// Simply return the text `OPOSSUM backend`. This is mostly for checking that the client is communication with the correct server.
#[utoipa::path(get, path="/", responses((status = OK, description = "Fixed answer string", body = str, example = "OPOSSUM backend")), tag="general")]
#[get("/")]
pub async fn hello() -> &'static str {
    "OPOSSUM backend"
}

/// Return a version information
///
/// Return the version numbers of the OPOSSUM library and the backend server.
#[utoipa::path(get, responses((status = OK, description = "Version information", body = VersionInfo)), tag="general")]
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
#[cfg(test)]
mod test {
    use super::*;
    use actix_web::{body::to_bytes, dev::Service, http::StatusCode, test, App};

    #[actix_web::test]
    async fn get_hello() {
        let app = test::init_service(App::new().service(hello)).await;
        let req = test::TestRequest::get().uri("/").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let response_body = resp.into_body();
        assert_eq!(to_bytes(response_body).await.unwrap(), "OPOSSUM backend");
    }
    #[actix_web::test]
    async fn get_version() {
        let app = test::init_service(App::new().service(version)).await;
        let req = test::TestRequest::get().uri("/version").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let _: VersionInfo = test::read_body_json(resp).await;
    }
}
