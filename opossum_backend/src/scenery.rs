use crate::app_state::AppState;
use actix_web::{
    get, post,
    web::{self, Data, Json},
    HttpResponse, Responder,
};
use log::{error, info};
use opossum::{meter, nodes::create_node_ref, optic_node::OpticNode, OpmDocument};
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

/// Get name of toplevel scenery
#[utoipa::path(responses((status = 200, body = str)), tag="scenery")]
#[get("/name")]
async fn name(data: web::Data<AppState>) -> impl Responder {
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
#[utoipa::path(tag = "node")]
#[post("/node")]
async fn add_node(data: web::Data<AppState>, node_type: String) -> impl Responder {
    if let Ok(node_ref) = create_node_ref(&node_type) {
        let mut scenery = data.scenery.lock().unwrap();
        if scenery.add_node_ref(node_ref.clone()).is_ok() {
            HttpResponse::Ok().json(node_ref)
        } else {
            HttpResponse::InternalServerError().json("Failed to add node")
        }
    } else {
        HttpResponse::BadRequest().body(format!("Node type '{node_type}' not found"))
    }
}
#[utoipa::path(tag = "node")]
#[get("/node/{uuid}")]
async fn get_node(data: web::Data<AppState>, uuid_str: web::Path<String>) -> impl Responder {
    let scenery = data.scenery.lock().unwrap();
    let Ok(uuid) = uuid_str.parse() else {
        return HttpResponse::BadRequest().body("Invalid UUID");
    };
    scenery.node(uuid).map_or_else(
        |_| HttpResponse::NotFound().body("Node not found"),
        |node| HttpResponse::Ok().json(node),
    )
}
#[derive(ToSchema, Deserialize)]
struct ConnectNodes {
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
    connect_info: Json<ConnectNodes>,
) -> impl Responder {
    let mut scenery = data.scenery.lock().unwrap();
    if let Err(e) = scenery.connect_nodes(
        connect_info.src_uuid,
        &connect_info.src_port,
        connect_info.target_uuid,
        &connect_info.target_port,
        meter!(connect_info.distance),
    ) {
        error!("error connecting nodes: {}", e.to_string());
        HttpResponse::BadRequest().body(e.to_string())
    } else {
        info!("nodes connected");
        HttpResponse::Ok().json("Nodes connected")
    }
}
#[utoipa::path(tag = "node")]
#[get("/opmfile")]
async fn get_opmfile(data: web::Data<AppState>) -> impl Responder {
    let scenery = data.scenery.lock().unwrap();
    let doc = OpmDocument::new(scenery.clone());
    drop(scenery);
    doc.to_opm_file_string().map_or_else(
        |_| HttpResponse::UnprocessableEntity().body("Error writing to file"),
        |doc_string| HttpResponse::Ok().body(doc_string),
    )
}
#[utoipa::path(tag = "node")]
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
            .service(name)
            .service(add_node)
            .service(get_node)
            .service(nr_of_nodes)
            .service(connect_nodes)
            .service(get_opmfile)
            .service(load_opmfile);
    }
}
