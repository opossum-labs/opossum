//! Routes for managing the scenery (top-level `NodeGroup`)
use crate::{
    app_state::AppState,
    error::ErrorResponse,
    nodes::{self},
};
use actix_web::{
    delete, get, http::StatusCode, post, web::{self, Json}, Error, HttpResponse, Responder
};
use nalgebra::Point2;
use opossum::{OpmDocument, SceneryResources, analyzers::AnalyzerType, opm_document::AnalyzerInfo};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

use actix_web::rt::time::interval;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use futures_util::StreamExt;

const RON_MEDIA_TYPE: &str = "application/ron";

/// Delete the current scenery and create new (empty) one
#[utoipa::path(responses((status = 200, description = "scenery deleted and new one sucessfully created")), tag="scenery")]
#[delete("/")]
async fn delete_scenery(data: web::Data<AppState>) -> impl Responder {
    let mut document = data.document.lock();
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
    let nr_of_nodes = data.document.lock().scenery().nr_of_nodes();
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
    let document = data.document.lock();
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
    data.document.lock().set_global_conf(global_conf.clone());
    web::Json(global_conf)
}
#[utoipa::path(tag = "scenery",
    responses((status = 200, description = "List of analyzers", body = Vec<AnalyzerInfo>)),
)]
/// Get a list of all analyzers of this model
///
/// This function returns a list of all analyzers of this model. Use the index to get a specific
/// analyzer.
#[get("/analyzers")]
async fn get_analyzers(data: web::Data<AppState>) -> impl Responder {
    let analyzers = data.document.lock().analyzers();
    let analyzers: Vec<AnalyzerInfo> = analyzers
        .values()
        .map(|a| {
            AnalyzerInfo::new(
                a.analyzer_type().clone(),
                a.id(),
                a.gui_position().map_or(Point2::new(0.0, 0.0), |p| p),
            )
        })
        .collect();
    web::Json(analyzers)
}
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct NewAnalyzerInfo {
    pub analyzer_type: AnalyzerType,
    pub gui_position: (f64, f64),
}

impl NewAnalyzerInfo {
    #[must_use]
    pub const fn new(analyzer_type: AnalyzerType, gui_position: (f64, f64)) -> Self {
        Self {
            analyzer_type,
            gui_position,
        }
    }
}
#[utoipa::path(tag = "scenery", 
    responses((status = 200, description = "Analyzer", body = AnalyzerType))
)]
/// Get an analyzer
///
/// This function returns the analyzer with the given index.
#[get("/analyzers/{Uuid}")]
async fn get_analyzer(
    data: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<Json<AnalyzerInfo>, ErrorResponse> {
    let analyzer_info = data.document.lock().analyzer(*id)?;
    Ok(Json(analyzer_info))
}
#[utoipa::path(tag = "scenery", request_body(content = NewAnalyzerInfo,
    description = "type and GUI position of node the analyzer to be created",
    content_type = "application/json",
    example ="{\"analyzer_type\": \"Energy\", \"gui_position\": [0,0,0]}"
),
    responses((status = 200, body = Uuid, )))]
/// Add an analyzer to the model
///
/// This function adds an analyzer to the model.
#[post("/analyzers")]
async fn add_analyzer(
    data: web::Data<AppState>,
    analyzer: web::Json<NewAnalyzerInfo>,
) -> impl Responder {
    let new_analyzer_info = analyzer.into_inner();
    let uuid = data.document.lock().add_analyzer_with_position(
        new_analyzer_info.analyzer_type,
        Some(new_analyzer_info.gui_position),
    );
    Json(uuid)
}
#[utoipa::path(tag = "scenery",
    responses((status = 200, description = "Analyzer deleted"),
    (status = 404, description = "Analyzer not found"))
)]
/// Delete an analyzer
///
/// This function deletes the analyzer with the given index.
#[delete("/analyzers/{uuid}")]
async fn delete_analyzer(
    data: web::Data<AppState>,
    index: web::Path<Uuid>,
) -> Result<&'static str, ErrorResponse> {
    let index = index.into_inner();
    data.document.lock().remove_analyzer(index)?;
    Ok("")
}
/// Get the OPM file as string
///
/// This function returns the OPM model file as string.
#[utoipa::path(tag = "scenery", 
    responses((status = 200, description = "OPM file", body = String, content_type=RON_MEDIA_TYPE))
)]
#[get("/opmfile")]
async fn get_opmfile(data: web::Data<AppState>) -> Result<impl Responder, ErrorResponse> {
    let document = data.document.lock();
    Ok(HttpResponse::Ok()
        .content_type(RON_MEDIA_TYPE)
        .body(document.to_opm_file_string()?))
}
#[utoipa::path(tag = "scenery", request_body(content = String,
    description = "OPM file as string",
    content_type = "text/plain",
),
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
    let mut document = data.document.lock();
    *document = OpmDocument::from_string(&opm_file_string)?;
    drop(document);
    Ok("")
}
#[utoipa::path(tag = "scenery", request_body(content = String,
    description = "Start a simulation run",
    content_type = "text/plain",
),
    responses((status = 200, description = "simulation sucessfully performed"),
    (status = 400, description = "Error during simulation"))
)]
/// Initiate an OPOSSUM simulation run
///
/// This function starts the simulation of the current scenery.
#[post("/simulate")]
async fn simulate(
    data: web::Data<AppState>,
    report_dir: String,
) -> impl Responder {
    let (tx, rx) = mpsc::channel(10);
    //let mut document = data.document.lock().clone();
    // Starte die "Simulation" in einem separaten Tokio-Task
    actix_web::rt::spawn(async move {
        //document.analyze();
        let mut count = 0;
        let mut ticker = interval(Duration::from_secs(1));

        loop {
            // Warte auf den nächsten "Tick"
            ticker.tick().await;
            count += 1;

            let log_message = format!("Simulationsschritt {} abgeschlossen.", count);

            // Sende die Log-Nachricht über den Channel
            // Wenn der Client die Verbindung schließt, schlägt dies fehl und wir beenden den Task.
            if tx.send(log_message).await.is_err() {
                println!("Client disconnected, stopping simulation.");
                break;
            }

            if count >= 10 { // Simulation nach 10 Schritten beenden
                let final_message = "Simulation beendet.".to_string();
                let _ = tx.send(final_message).await;
                println!("Simulation finished.");
                break;
            }
        }
    });

    // Erstelle einen Responder, der die Daten vom Channel als SSE-Stream sendet
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(ReceiverStream::new(rx).map(|s| -> Result<actix_web::web::Bytes, Error> {
            Ok(actix_web::web::Bytes::from(format!("data: {}\n\n", &s)))
        }))
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
    cfg.service(simulate);
    cfg.configure(nodes::config);
}
#[cfg(test)]
mod test {
    use actix_web::{App, dev::Service, test, web::Data};
    use opossum::{SceneryResources, nodes::Dummy};

    use crate::{app_state::AppState, scenery::NrOfNodes};

    #[actix_web::test]
    async fn nr_of_nodes_delete_scenery() {
        let app_state = Data::new(AppState::default());
        let mut document = app_state.document.lock();
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
