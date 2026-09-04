use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, dev::Server, middleware::Logger, web};
use std::net::Ipv4Addr;
use utoipa::OpenApi;
use utoipa_actix_web::AppExt;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    app_state::AppState, error::BackEndErrorResponse, pages, routes, sse_logger::init_logger,
};

/// Catch-All Handler für unbekannte Routen.
/// Gibt einen sauberen 404 Fehler im standardisierten API-Format zurück.
async fn not_found() -> HttpResponse {
    use actix_web::ResponseError; // Für die .error_response() Methode
    BackEndErrorResponse::not_found().error_response()
}

/// Initializes and starts the OPOSSUM Actix web server.
///
/// This function sets up the entire backend infrastructure:
/// - Initializes the global application state (`AppState`).
/// - Configures the Server-Sent Events (SSE) logger.
/// - Sets up the `OpenAPI` specification and Swagger UI.
/// - Configures Cross-Origin Resource Sharing (CORS).
/// - Registers all API endpoints via `routes::root_config`.
///
/// The server binds to the port specified by the `OPOSSUM_PORT` environment variable,
/// or defaults to `8001` if not set.
///
/// If configuration or binding fails, the process logs the error to stderr
/// and exits gracefully with error code 1.
pub fn start() -> Server {
    #[derive(OpenApi)]
    #[openapi(
        info(
            title = "OPOSSUM API", 
            description = "The REST API backend for the OPOSSUM optical simulation framework. It provides endpoints for creating, analyzing, and modifying optical models.", 
            contact(name="Udo Eisenbarth", email="u.eisenbarth@gsi.de"), 
            license(name="GPL-3.0")
        ),
        servers(
            (url = "http://localhost:8001", description = "Local desktop server"),
            (url = "https://example.com", description = "Production server (Optional)")
        ),
        tags(
            (name = "general", description = "General server endpoints (version, types, termination)."),
            (name = "node", description = "Endpoints for handling optical nodes, properties, and ports."),
            (name = "document", description = "Endpoints for managing the overall OPM model and global config."),
            (name = "analyzer", description = "Endpoints for managing simulation analyzers."),
            (name = "pump_scenario", description = "Endpoints for managing pump scenarios (amplifier operating points)."),
            (name = "operations", description = "Complex macro-operations (e.g., copy, paste, grouping)."),
        ),
    )]
    pub struct ApiDocs;

    init_logger();
    let app_state = web::Data::new(AppState::default());

    // Read OPOSSUM_PORT safely. Exit with error if the value is invalid.
    let port: u16 = std::env::var("OPOSSUM_PORT").map_or(8001, |val| {
        val.parse().unwrap_or_else(|e| {
            eprintln!("Invalid OPOSSUM_PORT environment variable '{val}': {e}");
            std::process::exit(1);
        })
    });

    // Read OPOSSUM_WORKERS safely. Exit with error if the value is invalid.
    let workers: usize = std::env::var("OPOSSUM_WORKERS").map_or(2, |val| {
        val.parse().unwrap_or_else(|e| {
            eprintln!("Invalid OPOSSUM_WORKERS environment variable '{val}': {e}");
            std::process::exit(1);
        })
    });

    let srv = HttpServer::new({
        let app_state = app_state.clone();
        move || {
            // CORS Configuration: Fix later for production use.
            // Optional: Limit e.g. with .allowed_origin("http://localhost:8080")
            let cors = Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
                .max_age(3600);

            App::new()
                .into_utoipa_app()
                .openapi(ApiDocs::openapi())
                .map(|app| app.wrap(Logger::default()))
                .map(|app| app.wrap(cors))
                .app_data(app_state.clone())
                .configure(routes::root_config)
                .openapi_service(|api| {
                    SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", api)
                })
                .service(pages::welcome)
                .default_service(web::route().to(not_found))
                .into_app()
        }
    })
    .workers(workers)
    .bind((Ipv4Addr::UNSPECIFIED, port))
    .unwrap_or_else(|e| {
        // Exit gracefully with error code 1 instead of panicking.
        eprintln!("Failed to bind server to port {port}: {e}");
        std::process::exit(1);
    })
    .run();

    app_state.register_server_handle(srv.handle());

    srv
}
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, dev::Service, http::StatusCode, test};
    use opossum_core::types::api_types::ErrorResponse;

    #[actix_web::test]
    async fn test_not_found_handler_returns_formatted_error() {
        let app = test::init_service(App::new().default_service(web::route().to(not_found))).await;

        let req = test::TestRequest::get()
            .uri("/unmapped_endpoint")
            .to_request();
        let resp = app.call(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let error_body: ErrorResponse = test::read_body_json(resp).await;
        assert_eq!(error_body.status, 404);
        assert_eq!(error_body.category, "NotFound");
    }
}
