//! Routes for managing the document
use crate::{app_state::AppState, error::BackEndErrorResponse, sse_logger::SENDER};
use actix_web::{
    Error, HttpResponse, Responder, delete, get,
    http::StatusCode,
    patch, post, put,
    web::{self, Json},
};
use futures_util::StreamExt;
use log::{error, info, warn};
use opossum_core::{
    core_optics::{OpticNode, SceneryResources},
    opm_document::OpmDocument,
};
use std::{path::PathBuf, str::FromStr};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use utoipa_actix_web::service_config::ServiceConfig;

const RON_MEDIA_TYPE: &str = "application/ron";

/// Delete the current document and create new (empty) one
#[utoipa::path(responses((status = 200, description = "document deleted and new one sucessfully created")), tag="document")]
#[delete("/")]
async fn delete_document(data: web::Data<AppState>) -> impl Responder {
    let mut document = data.document.lock();
    *document = OpmDocument::default();
    drop(document);
    HttpResponse::new(StatusCode::OK)
}
#[utoipa::path(tag = "document",
    responses((status = 200, description = "Global configuration", body = SceneryResources))
)]
/// Get the global configuration of this model
///
/// This function returns the global configuration of the model.
#[get("/global_conf")]
async fn get_global_conf(data: web::Data<AppState>) -> impl Responder {
    let document = data.document.lock();
    web::Json(document.global_conf().lock().unwrap().clone())
}

#[utoipa::path(tag = "document",
    responses((status = 200, description = "Global configuration", body = SceneryResources))
)]
/// Set the global configuration
///
/// This function sets the global configuration of the model. The old global configuration is
/// replaced by the new one.
#[patch("/global_conf")]
async fn patch_global_conf(
    data: web::Data<AppState>,
    new_global_conf: web::Json<SceneryResources>,
) -> impl Responder {
    let global_conf = new_global_conf.into_inner();
    data.document.lock().set_global_conf(global_conf.clone());
    web::Json(global_conf)
}
#[utoipa::path(tag = "document",
    responses((status = 200, description = "Scenery Uuid", body = SceneryResources))
)]
/// Get the uuid of the root node of this model
///
/// This function returns the uuid of the root node (group) of the document.
#[get("/root_uuid")]
async fn get_root_uuid(data: web::Data<AppState>) -> impl Responder {
    let document = data.document.lock();
    web::Json(document.scenery().node_attr().uuid())
}
/// Get the document as an (OPM file) string
///
/// This function returns the entire document as an OPM model file string.
#[utoipa::path(tag = "document", 
    responses((status = 200, description = "OPM file", body = String, content_type=RON_MEDIA_TYPE))
)]
#[get("/")]
async fn get_document(data: web::Data<AppState>) -> Result<impl Responder, BackEndErrorResponse> {
    let document = data.document.lock();
    Ok(HttpResponse::Ok()
        .content_type(RON_MEDIA_TYPE)
        .body(document.to_opm_file_string()?))
}
#[utoipa::path(tag = "document", request_body(content = String,
    description = "OPM file as string",
    content_type = "text/plain",
),
    responses((status = 200, description = "OPM file sucessfully parsed"),
    (status = 400, description = "Error parsing OPM file"))
)]
/// Load a document from an OPM file string
///
/// This function reads a OPM model from the given OPM file string and replaces the current
/// document.
#[put("/")]
async fn put_document(
    data: web::Data<AppState>,
    opm_file_string: String,
) -> Result<Json<String>, BackEndErrorResponse> {
    let mut document = data.document.lock();
    *document = OpmDocument::from_string(&opm_file_string)?;
    let name = document.scenery().node_attr().name();
    drop(document);
    Ok(Json(name))
}

#[utoipa::path(tag = "document", request_body(content = String,
    description = "Start a simulation run",
    content_type = "text/plain",
),
    responses((status = 200, description = "simulation sucessfully performed"))
)]
/// Initiate an OPOSSUM simulation run
///
/// This function starts the simulation of the current document.
#[post("/simulate")]
async fn simulate(data: web::Data<AppState>, report_dir: String) -> impl Responder {
    let (tx, rx) = mpsc::channel(10);
    let mut document = data.document.lock().clone();
    // Run the synchronous, blocking code in a dedicated thread pool.
    web::block(move || {
        SENDER.with(|cell| {
            *cell.borrow_mut() = Some(tx);
        });
        match PathBuf::from_str(&report_dir) {
            Ok(report_dir) => {
                info!("Creating report directory: {}", report_dir.display());
                // if let Err(e) = recreate_data_dir(&report_dir) {
                //     error!("Error creating data directory: {e}");
                // } else {
                info!("Creating diagram files");
                document
                    .create_dot_file(&report_dir)
                    .unwrap_or_else(|e| warn!("{e}"));
                info!("Starting analysis");
                let analysis_reports = document.analyze();
                match analysis_reports {
                    Ok(reports) => {
                        info!("Generating report(s)");
                        for report in reports.iter().enumerate() {
                            report
                                .1
                                .save(&report_dir, report.0)
                                .unwrap_or_else(|e| warn!("{e}"));
                        }
                    }
                    Err(e) => {
                        error!("Error during analysis: {e}");
                    } // }
                }
            }
            Err(e) => {
                error!("Ill-formatted report directory: {e}");
            }
        }
        SENDER.with(|cell| {
            *cell.borrow_mut() = None;
        });
    })
    .await
    .ok(); // We don't care about the result of block, just that it ran.
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(
            ReceiverStream::new(rx).map(|s| -> Result<actix_web::web::Bytes, Error> {
                Ok(actix_web::web::Bytes::from(format!("data: {}\n\n", &s)))
            }),
        )
}
pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_document);
    cfg.service(put_document);
    cfg.service(delete_document);

    cfg.service(get_global_conf);
    cfg.service(patch_global_conf);

    cfg.service(get_root_uuid);

    // cfg.service(simulate);
    //cfg.configure(nodes::config);
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::app_state::AppState;
    use actix_web::{App, dev::Service, test, web::Data};
    use opossum_core::core_optics::SceneryResources;

    #[actix_web::test]
    async fn test_get_global_conf() {
        let app_state = Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(get_global_conf)
                .service(patch_global_conf),
        )
        .await;
        let req = test::TestRequest::get().uri("/global_conf").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), 200);
        let _: SceneryResources = test::read_body_json(resp).await; // Panics, if not valid JSON
    }
}
