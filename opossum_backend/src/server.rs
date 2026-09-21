// In opossum_backend/src/server.rs

use actix_cors::Cors;
use actix_web::{
    dev::Server,
    middleware::{Condition, Logger},
    web, App, HttpResponse, HttpServer,
};
use std::net::Ipv4Addr;
use utoipa::OpenApi;
use utoipa_actix_web::AppExt;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    app_state::AppState, error::BackEndErrorResponse, pages, payload_logger::PayloadLogger,
    routes, sse_logger::init_logger,
};

#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    pub debug_payload: bool,
}

async fn not_found() -> HttpResponse {
    use actix_web::ResponseError;
    BackEndErrorResponse::not_found().error_response()
}

pub fn start_with_config(config: ServerConfig) -> Server {
    #[derive(OpenApi)]
    #[openapi(
        info(
            title = "OPOSSUM API",
            description = "The REST API backend for the OPOSSUM optical simulation framework. It provides endpoints for creating, analyzing, and modifying optical models.",
            contact(name = "Udo Eisenbarth", email = "u.eisenbarth@gsi.de"),
            license(name = "GPL-3.0")
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
        )
    )]
    pub struct ApiDocs;

    init_logger();
    let app_state = web::Data::new(AppState::default());

    let port: u16 = std::env::var("OPOSSUM_PORT").map_or(8001, |val| {
        val.parse().unwrap_or_else(|e| {
            eprintln!("Invalid OPOSSUM_PORT environment variable '{val}': {e}");
            std::process::exit(1);
        })
    });

    // Enforce 1 worker in debug mode to eliminate multi-threaded stdout race conditions
    let workers: usize = if config.debug_payload {
        1
    } else {
        std::env::var("OPOSSUM_WORKERS").map_or(2, |val| {
            val.parse().unwrap_or_else(|e| {
                eprintln!("Invalid OPOSSUM_WORKERS environment variable '{val}': {e}");
                std::process::exit(1);
            })
        })
    };

    // Instantiate PayloadLogger outside the closure so shared lock state is preserved
    let payload_logger = PayloadLogger::new();

    let srv = HttpServer::new({
        let app_state = app_state.clone();
        let debug_payload = config.debug_payload;
        let payload_logger = payload_logger.clone();

        move || {
            let cors = Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
                .max_age(3600);

            App::new()
                .into_utoipa_app()
                .openapi(ApiDocs::openapi())
                // Standard logger is active ONLY when debug_payload is OFF
                .map(|app| app.wrap(Condition::new(!debug_payload, Logger::default())))
                // Detailed sequential payload logger is active ONLY when debug_payload is ON
                .map(|app| app.wrap(Condition::new(debug_payload, payload_logger.clone())))
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
        eprintln!("Failed to bind server to port {port}: {e}");
        std::process::exit(1);
    })
    .run();

    app_state.register_server_handle(srv.handle());

    srv
}
