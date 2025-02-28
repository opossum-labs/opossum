use actix_cors::Cors;
use actix_web::{
    dev::Server, http::StatusCode, middleware::Logger, web, App, HttpResponse, HttpServer,
};
use env_logger::Env;
use std::net::Ipv4Addr;
use utoipa::OpenApi;
use utoipa_actix_web::AppExt;
use utoipa_swagger_ui::SwaggerUi;

use crate::{app_state::AppState, general, scenery};

async fn not_found() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::build(StatusCode::NOT_FOUND)
        .content_type("text/html; charset=utf-8")
        .body("<h1>Error 404</h1>"))
}

/// Start the API server.
///
/// # Panics
///
/// Panics if the server could not be bind to a port.
pub fn start() -> Server {
    #[derive(OpenApi)]
    #[openapi(
        info(title = "OPOSSUM API", description = "Description blah blah...", contact(name="Udo Eisenbarth", email="u.eisenbarth@gsi.de"), license(name="GPL3")),
        servers(
            (url= "http://localhost:8001", description = "local development server"),
            (url="https://example.com", description ="production server")
        ),
        tags(
            (name = "general", description = "general endpoints."),
            (name = "scenery", description = "endpoints dealing with the toplevel scenery."),
            (name = "node", description = "endpoints dealing with handling of optical nodes."),
        )
    )]
    pub struct ApiDoc;

    env_logger::init_from_env(Env::default().default_filter_or("info"));
    let app_state = web::Data::new(AppState::default());
    HttpServer::new(move || {
        App::new()
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            .map(|app| app.wrap(Logger::default()))
            .map(|app| app.wrap(Cors::permissive())) // change this in production !!!
            .app_data(app_state.clone())
            .service(
                utoipa_actix_web::scope("/api/scenery")
                    .configure(scenery::configure(app_state.clone())),
            )
            .service(
                utoipa_actix_web::scope("/api").configure(general::configure(app_state.clone())),
            )
            .openapi_service(|api| {
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", api)
            })
            .default_service(web::route().to(not_found))
            .into_app()
    })
    .bind((Ipv4Addr::UNSPECIFIED, 8001))
    .expect("Failed to bind server")
    .run()
}
