//! Routes for managing the scenery (top-level `NodeGroup`)
use crate::{app_state::AppState, error::ErrorResponse, nodes};
use actix_web::{
    delete, get, post,
    web::{self},
    HttpResponse, Responder,
};
use opossum::{analyzers::AnalyzerType, optic_node::OpticNode, OpmDocument, SceneryResources};
use utoipa_actix_web::service_config::ServiceConfig;

/// Create new (empty) scenery ([`OpmDocument`])
#[utoipa::path(responses((status = 200, body = str)), tag="scenery")]
#[post("/")]
async fn post_scenery(data: web::Data<AppState>) -> impl Responder {
    let mut document = data.document.lock().unwrap();
    *document = OpmDocument::default();
    ""
}
/// Get name of toplevel scenery
#[utoipa::path(responses((status = 200, body = str)), tag="scenery")]
#[get("/name")]
async fn get_name(data: web::Data<AppState>) -> impl Responder {
    let name = data.document.lock().unwrap().scenery().name();
    HttpResponse::Ok().body(name)
}
/// Get number of toplevel nodes
#[utoipa::path(get, responses((status = 200, body = str)), tag="scenery")]
#[get("/nr_of_nodes")]
async fn nr_of_nodes(data: web::Data<AppState>) -> impl Responder {
    let nr_of_nodes = data.document.lock().unwrap().scenery().nr_of_nodes();
    HttpResponse::Ok().body(nr_of_nodes.to_string())
}
#[utoipa::path(tag = "scenery")]
#[get("/global_conf")]
#[allow(clippy::significant_drop_tightening)] // no idea, how to fix this ...
async fn get_global_conf(data: web::Data<AppState>) -> impl Responder {
    let document = data.document.lock().unwrap();
    let global_conf = document.global_conf();
    HttpResponse::Ok().json(global_conf)
}
#[utoipa::path(tag = "scenery")]
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
    HttpResponse::Ok().json(global_conf)
}
#[utoipa::path(tag = "scenery")]
#[get("/analyzers")]
async fn get_analyzers(data: web::Data<AppState>) -> impl Responder {
    let analyzers = data.document.lock().unwrap().analyzers();
    web::Json(analyzers)
}
#[utoipa::path(tag = "scenery")]
#[get("/analyzers/{index}")]
async fn get_analyzer(data: web::Data<AppState>, index: web::Path<usize>) -> impl Responder {
    let analyzers = data.document.lock().unwrap().analyzers();
    analyzers.get(*index).map_or_else(
        || HttpResponse::NotFound().body("Analyzer not found"),
        |analyzer| HttpResponse::Ok().json(analyzer),
    )
}
#[utoipa::path(tag = "scenery")]
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
#[utoipa::path(tag = "scenery")]
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
#[utoipa::path(tag = "scenery")]
#[get("/opmfile")]
async fn get_opmfile(data: web::Data<AppState>) -> Result<String, ErrorResponse> {
    let document = data.document.lock().unwrap();
    Ok(document.to_opm_file_string()?)
}
#[utoipa::path(tag = "scenery")]
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
    cfg.service(post_scenery);
    cfg.service(get_name);
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
