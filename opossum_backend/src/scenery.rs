//! Routes for managing the scenery (top-level `NodeGroup`)
use crate::{app_state::AppState, error::ErrorResponse};
use actix_web::{
    delete, get, post, put,
    web::{self, Data, Json},
    HttpResponse, Responder,
};
use opossum::{
    analyzers::AnalyzerType, meter, nodes::create_node_ref, optic_node::OpticNode,
    optic_ref::OpticRef, OpmDocument, SceneryResources,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

/// Get name of toplevel scenery
#[utoipa::path(responses((status = 200, body = str)), tag="scenery")]
#[get("/name")]
async fn get_name(data: web::Data<AppState>) -> impl Responder {
    let scenery = data.scenery.lock().unwrap();
    HttpResponse::Ok().body(scenery.name())
}
/// Get number of toplevel nodes
#[utoipa::path(get, responses((status = 200, body = str)), tag="scenery")]
#[get("/nr_of_nodes")]
async fn nr_of_nodes(data: web::Data<AppState>) -> impl Responder {
    let scenery = data.scenery.lock().unwrap();
    HttpResponse::Ok().body(scenery.nr_of_nodes().to_string())
}
#[utoipa::path(tag = "scenery")]
#[get("/global_conf")]
async fn get_global_conf(data: web::Data<AppState>) -> impl Responder {
    let global_conf = data.global_conf.lock().unwrap();
    HttpResponse::Ok().json(global_conf.clone())
}
#[utoipa::path(tag = "scenery")]
#[post("/global_conf")]
async fn post_global_conf(
    data: web::Data<AppState>,
    new_global_conf: web::Json<SceneryResources>,
) -> impl Responder {
    let mut global_conf = data.global_conf.lock().unwrap();
    *global_conf = new_global_conf.into_inner();
    HttpResponse::Ok().json(global_conf.clone())
}
#[utoipa::path(tag = "scenery")]
#[get("/analyzers")]
async fn get_analyzers(data: web::Data<AppState>) -> impl Responder {
    let analyzers = data.analyzers.lock().unwrap();
    web::Json(analyzers.clone())
}
#[utoipa::path(tag = "scenery")]
#[get("/analyzers/{index}")]
async fn get_analyzer(data: web::Data<AppState>, index: web::Path<usize>) -> impl Responder {
    let analyzers = data.analyzers.lock().unwrap();
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
    let mut analyzers = data.analyzers.lock().unwrap();
    analyzers.push(analyzer.into_inner());
    web::Json(analyzers.clone())
}
#[utoipa::path(tag = "scenery")]
#[delete("/analyzers/{index}")]
async fn delete_analyzer(data: web::Data<AppState>, _index: web::Path<usize>) -> impl Responder {
    let mut _analyzers = data.analyzers.lock().unwrap();
    // if analyzers.remove(*index).is_some() {
    //     HttpResponse::Ok().json(analyzers.clone())
    // } else {
    HttpResponse::NotFound().body("Analyzer not found")
    // }
}
/// Add a new node to the toplevel scenery
#[utoipa::path(tag = "node")]
#[post("/nodes")]
async fn add_node(
    data: web::Data<AppState>,
    node_type: String,
) -> Result<Json<OpticRef>, ErrorResponse> {
    let node_ref = create_node_ref(&node_type)?;
    let mut scenery = data.scenery.lock().unwrap();
    scenery.add_node_ref(node_ref.clone())?;
    drop(scenery);
    Ok(web::Json(node_ref))
}
#[utoipa::path(tag = "node")]
#[get("/nodes/{uuid}")]
async fn get_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<OpticRef>, ErrorResponse> {
    let uuid = path.into_inner();
    let scenery = data.scenery.lock().unwrap();
    let node_ref = scenery.node(uuid)?;
    drop(scenery);
    Ok(web::Json(node_ref))
}
#[utoipa::path(tag = "node")]
#[put("/nodes/{uuid}")]
async fn put_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<OpticRef>, ErrorResponse> {
    let uuid = path.into_inner();
    let scenery = data.scenery.lock().unwrap();
    let node_ref = scenery.node(uuid)?;
    drop(scenery);
    Ok(web::Json(node_ref))
}
#[utoipa::path(tag = "node")]
#[delete("/nodes/{uuid}")]
async fn delete_node(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, ErrorResponse> {
    let uuid = path.into_inner();
    let mut scenery = data.scenery.lock().unwrap();
    let deleted_nodes = scenery.delete_node(uuid)?;
    drop(scenery);
    Ok(web::Json(deleted_nodes))
}
#[derive(ToSchema, Serialize, Deserialize)]
struct ConnectInfo {
    src_uuid: Uuid,
    src_port: String,
    target_uuid: Uuid,
    target_port: String,
    distance: f64,
}
#[utoipa::path(tag = "node")]
#[post("/connect")]
async fn connect_nodes(
    data: web::Data<AppState>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, ErrorResponse> {
    let mut scenery = data.scenery.lock().unwrap();
    scenery.connect_nodes(
        connect_info.src_uuid,
        &connect_info.src_port,
        connect_info.target_uuid,
        &connect_info.target_port,
        meter!(connect_info.distance),
    )?;
    Ok(connect_info)
}
#[utoipa::path(tag = "scenery")]
#[get("/opmfile")]
async fn get_opmfile(data: web::Data<AppState>) -> Result<String, ErrorResponse> {
    let scenery = data.scenery.lock().unwrap();
    let doc = OpmDocument::new(scenery.clone());
    drop(scenery);
    Ok(doc.to_opm_file_string()?)
}
#[utoipa::path(tag = "scenery")]
#[post("/opmfile")]
async fn load_opmfile(data: web::Data<AppState>, opm_file_string: String) -> impl Responder {
    match &mut OpmDocument::from_string(&opm_file_string) {
        Ok(doc) => {
            let mut scenery = data.scenery.lock().unwrap();
            doc.scenery_mut().clone_into(&mut scenery);
            HttpResponse::Ok().body("")
        }
        Err(e) => HttpResponse::UnprocessableEntity().body(format!("Error reading file: {e}")),
    }
}

pub fn configure(store: Data<AppState>) -> impl FnOnce(&mut ServiceConfig<'_>) {
    |config: &mut ServiceConfig<'_>| {
        config
            .app_data(store)
            .service(get_name)
            .service(get_global_conf)
            .service(post_global_conf)
            .service(get_analyzers)
            .service(get_analyzer)
            .service(add_analyzer)
            .service(delete_analyzer)
            .service(nr_of_nodes)
            .service(add_node)
            .service(get_node)
            .service(put_node)
            .service(delete_node)
            .service(connect_nodes)
            .service(get_opmfile)
            .service(load_opmfile);
    }
}
