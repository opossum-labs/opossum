//! Routes for managing the scenery (top-level `NodeGroup`)
use crate::{app_state::AppState, error::ErrorResponse, nodes};
use actix_web::{
    delete, get,
    http::StatusCode,
    post,
    web::{self},
    HttpResponse, Responder,
};
use opossum::{analyzers::AnalyzerType, OpmDocument, SceneryResources};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;

/// Delete the current scenery and create new (empty) one
#[utoipa::path(responses((status = 200, description = "scenery deleted and new one sucessfully created")), tag="scenery")]
#[delete("/")]
async fn delete_scenery(data: web::Data<AppState>) -> impl Responder {
    let mut document = data.document.lock().unwrap();
    *document = OpmDocument::default();
    drop(document);
    HttpResponse::new(StatusCode::OK)
}
#[derive(Serialize, Deserialize, ToSchema)]
struct NrOfNodes(usize);
/// Get number of toplevel nodes
#[utoipa::path(get, responses((status = 200, body = NrOfNodes)), tag="scenery")]
/// Get the number of toplevel nodes
///
/// This function returns the number of toplevel nodes in the scenery. This function is mainly used
/// for testing purposes.
#[get("/nr_of_nodes")]
async fn nr_of_nodes(data: web::Data<AppState>) -> impl Responder {
    let nr_of_nodes = data.document.lock().unwrap().scenery().nr_of_nodes();
    web::Json(NrOfNodes(nr_of_nodes))
}
#[utoipa::path(tag = "scenery",
    responses((status = 200, description = "Global configuration", body = SceneryResources))
)]
/// Get the global configuration of this model
///
/// This function returns the global configuration of the model.
#[get("/global_conf")]
#[allow(clippy::significant_drop_tightening)] // no idea, how to fix this ...
async fn get_global_conf(data: web::Data<AppState>) -> impl Responder {
    let document = data.document.lock().unwrap();
    let global_conf = document.global_conf().lock().unwrap().clone();
    web::Json(global_conf)
}
#[utoipa::path(tag = "scenery",
    responses((status = 200, description = "Global configuration", body = SceneryResources))
)]
/// Set the global configuration
///
/// This function sets the global configuration of the model. The old global configuration is
/// replaced by the new one.
#[post("/global_conf")]
async fn post_global_conf(
    data: web::Data<AppState>,
    new_global_conf: web::Json<SceneryResources>,
) -> impl Responder {
    let global_conf = new_global_conf.into_inner();
    data.document
        .lock()
        .unwrap()
        .set_global_conf(global_conf.clone());
    web::Json(global_conf)
}
#[utoipa::path(tag = "scenery",
    responses((status = 200, description = "List of analyzers", body = Vec<AnalyzerType>)),
)]
/// Get a list of all analyzers of this model
///
/// This function returns a list of all analyzers of this model. Use the index to get a specific
/// analyzer.
#[get("/analyzers")]
async fn get_analyzers(data: web::Data<AppState>) -> impl Responder {
    let analyzers = data.document.lock().unwrap().analyzers();
    web::Json(analyzers)
}
#[utoipa::path(tag = "scenery", 
    responses((status = 200, description = "Analyzer", body = AnalyzerType))
)]
/// Get an analyzer
///
/// This function returns the analyzer with the given index.
#[get("/analyzers/{index}")]
async fn get_analyzer(data: web::Data<AppState>, index: web::Path<usize>) -> impl Responder {
    let analyzers = data.document.lock().unwrap().analyzers();
    analyzers.get(*index).map_or_else(
        || HttpResponse::NotFound().body("Analyzer not found"),
        |analyzer| HttpResponse::Ok().json(analyzer),
    )
}
#[utoipa::path(tag = "scenery", 
    responses((status = 200, body = Vec<AnalyzerType>)))]
/// Add an analyzer to the model
///
/// This function adds an analyzer to the model.
#[post("/analyzers")]
async fn add_analyzer(
    data: web::Data<AppState>,
    analyzer: web::Json<AnalyzerType>,
) -> impl Responder {
    let analyzer = analyzer.into_inner();
    let mut document = data.document.lock().unwrap();
    document.add_analyzer(analyzer);
    web::Json(document.analyzers())
}
#[utoipa::path(tag = "scenery",
    responses((status = 200, description = "Analyzer deleted"),
    (status = 404, description = "Analyzer not found"))
)]
/// Delete an analyzer
///
/// This function deletes the analyzer with the given index.
#[delete("/analyzers/{index}")]
async fn delete_analyzer(
    data: web::Data<AppState>,
    index: web::Path<usize>,
) -> Result<&'static str, ErrorResponse> {
    let index = index.into_inner();
    let mut document = data.document.lock().unwrap();
    document.remove_analyzer(index)?;
    drop(document);
    Ok("")
}
/// Get the OPM file as string
///
/// This function returns the OPM model file as string.
#[utoipa::path(tag = "scenery", 
    responses((status = 200, description = "OPM file", body = String))
)]
#[get("/opmfile")]
async fn get_opmfile(data: web::Data<AppState>) -> Result<String, ErrorResponse> {
    let document = data.document.lock().unwrap();
    Ok(document.to_opm_file_string()?)
}
#[utoipa::path(tag = "scenery",
    responses((status = 200, description = "OPM file sucessfully parsed"),
    (status = 400, description = "Error parsing OPM file"))
)]
/// Load an OPM file
///
/// This function reads a OPM model from the given OPM file string and replaces the current
/// scenery.
#[post("/opmfile")]
async fn post_opmfile(
    data: web::Data<AppState>,
    opm_file_string: String,
) -> Result<&'static str, ErrorResponse> {
    let mut document = data.document.lock().unwrap();
    *document = OpmDocument::from_string(&opm_file_string)?;
    drop(document);
    Ok("")
}
pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(delete_scenery);
    cfg.service(get_global_conf);
    cfg.service(post_global_conf);
    cfg.service(get_analyzers);
    cfg.service(get_analyzer);
    cfg.service(add_analyzer);
    cfg.service(delete_analyzer);
    cfg.service(nr_of_nodes);
    cfg.service(get_opmfile);
    cfg.service(post_opmfile);
    cfg.configure(nodes::config);
}
#[cfg(test)]
mod test {
    use actix_web::{dev::Service, test, web::Data, App};
    use opossum::{nodes::Dummy, SceneryResources};

    use crate::{app_state::AppState, scenery::NrOfNodes};

    #[actix_web::test]
    async fn nr_of_nodes_delete_scenery() {
        let app_state = Data::new(AppState::default());
        let mut document = app_state.document.lock().unwrap();
        let scenery = document.scenery_mut();
        scenery.add_node(Dummy::default()).unwrap();
        drop(document);
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(super::delete_scenery)
                .service(super::nr_of_nodes),
        )
        .await;
        let req = test::TestRequest::get().uri("/nr_of_nodes").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let nr_of_nodes: NrOfNodes = test::read_body_json(resp).await;
        assert_eq!(nr_of_nodes.0, 1);
        let req = test::TestRequest::delete().uri("/").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let req = test::TestRequest::get().uri("/nr_of_nodes").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let nr_of_nodes: NrOfNodes = test::read_body_json(resp).await;
        assert_eq!(nr_of_nodes.0, 0);
    }
    #[actix_web::test]
    async fn get_global_conf() {
        let app_state = Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(super::get_global_conf)
                .service(super::post_global_conf),
        )
        .await;
        let req = test::TestRequest::get().uri("/global_conf").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let _: SceneryResources = test::read_body_json(resp).await; // Panics, if not valid JSON
    }
}
