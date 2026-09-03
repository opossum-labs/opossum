use actix_web::{
    HttpRequest, HttpResponse, delete, get, patch, post, put,
    web::{self},
};
use opossum_core::{
    core_optics::NodeAttr,
    distributions::spectral::{LaserLines, SpecDistType},
    joule,
    light::lightdata::{
        energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
        ray_data_builder::RayDataBuilder,
        ray_data_source::RayDataSource,
    },
    nanometer,
    nodes::NodeGroup,
    opm_document::AnalyzerInfo,
    prelude::AnalyzerType,
    types::api_types::{AnalyzerItemDto, ErrorResponse, NewAnalyzerInfo, SourcePortDto},
};
use uom::si::f64::Length;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{
        Ron, analyzer_mut_or_404, apply_and_push_undo, collect_nodes, ron_or_json_response,
    },
    undo::{Command, PatchAnalyzer, PatchAnalyzerPumpScenarios, RepositionAnalyzer},
};

/// Collects every "source port" node of the whole document as `(uuid, name)` pairs, in depth-first
/// order.
fn collect_source_ports(scenery: &NodeGroup) -> Vec<(Uuid, String)> {
    collect_nodes(scenery, &|node_attr: &NodeAttr| {
        (node_attr.node_type() == "source port").then(|| node_attr.name().to_string())
    })
    .into_iter()
    .map(|node| (node.uuid, node.value))
    .collect()
}

/// Get an analyzer by UUID
///
/// Returns all information (`AnalyzerInfo`) of the analyzer specified by its UUID.
/// The format is determined by the `Accept` header (`application/ron` or `application/json`).
/// Defaults to JSON.
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    responses(
        (status = OK, description = "Analyzer information successfully retrieved", content((AnalyzerInfo = "application/json"),( AnalyzerInfo="application/ron"))),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[get("/{uuid}")]
#[allow(clippy::future_not_send)]
pub async fn get_analyzer(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let analyzer_info = get_node_analyzer_attr_from_state(uuid, &data)?;
    ron_or_json_response(&req, &analyzer_info)
}

fn get_node_analyzer_attr_from_state(
    uuid: Uuid,
    data: &web::Data<AppState>,
) -> Result<AnalyzerInfo, BackEndErrorResponse> {
    let document = data.document.lock().clone();
    let analyzer_info = document
        .analyzers()
        .get(&uuid)
        .ok_or_else(BackEndErrorResponse::analyzer_not_found)?
        .clone();
    Ok(analyzer_info)
}

/// Creates a default `EnergyDataBuilder` optionally configured with a custom wavelength.
pub fn create_default_energy_builder(default_wvl: Option<Length>) -> EnergyDataBuilder {
    match default_wvl {
        Some(wvl) => {
            let ell = EnergyLaserLines::new(vec![(wvl, joule!(1.0))], nanometer!(0.1))
                .unwrap_or_default();
            EnergyDataBuilder::LaserLines(ell)
        }
        None => EnergyDataBuilder::default(),
    }
}

/// Creates a default `RayDataBuilder` optionally configured with a custom wavelength.
pub fn create_default_ray_builder(default_wvl: Option<Length>) -> RayDataBuilder {
    match default_wvl {
        Some(wvl) => {
            let mut rds = RayDataSource::default();
            if let Ok(lines) = LaserLines::new(vec![(wvl, 1.0)]) {
                rds.set_spectral_dist(SpecDistType::LaserLines(lines));
            }
            RayDataBuilder::from(rds)
        }
        None => RayDataBuilder::default(),
    }
}

/// Add an analyzer to the model
#[utoipa::path(
    tag = "analyzer", 
    request_body(content = NewAnalyzerInfo, description = "type and GUI position of the analyzer to be created", content_type = "application/json", example ="{\"analyzer_type\": \"Energy\", \"gui_position\": [0,0,0]}"),
    responses((status = CREATED, body = Uuid, description = "Analyzer successfully created"))
)]
#[post("")]
pub async fn post_analyzer(
    data: web::Data<AppState>,
    analyzer: web::Json<NewAnalyzerInfo>,
) -> HttpResponse {
    let new_analyzer_info = analyzer.into_inner();
    let default_wvl = new_analyzer_info.default_wavelength;
    let mut document = data.document.lock();

    // 1. Create the analyzer core instance
    let uuid = document.add_analyzer_with_position(
        new_analyzer_info.analyzer_type,
        Some(new_analyzer_info.gui_position),
    );

    // 2. Automatically populate default mappings for all currently existing source ports
    let source_uuids: Vec<Uuid> = collect_source_ports(document.scenery())
        .into_iter()
        .map(|(uuid, _)| uuid)
        .collect();
    if let Some(analyzer_info) = document.analyzer_mut(uuid) {
        // Persist the default wavelength in the analyzer instance
        analyzer_info.set_default_wavelength(default_wvl);

        let mut a_type = analyzer_info.analyzer_type().clone();
        for port_uuid in source_uuids {
            match &mut a_type {
                AnalyzerType::Energy(config) => {
                    config.map_source(port_uuid, create_default_energy_builder(default_wvl));
                }
                AnalyzerType::RayTrace(config) => {
                    config.map_source(port_uuid, create_default_ray_builder(default_wvl));
                }
                AnalyzerType::GhostFocus(config) => {
                    config.map_source(port_uuid, create_default_ray_builder(default_wvl));
                }
            }
        }
        analyzer_info.set_analyzer_type(&a_type);
    }

    if let Ok(info) = document.analyzer(uuid) {
        data.push_undo(Command::RemoveAnalyzer(AnalyzerItemDto { id: uuid, info }));
    }
    drop(document);
    HttpResponse::Created().json(uuid)
}

/// Delete an analyzer
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer to delete")),
    responses(
        (status = NO_CONTENT, description = "Analyzer deleted"),
        (status = NOT_FOUND, body = ErrorResponse, description = "Analyzer not found")
    )
)]
#[delete("/{uuid}")]
pub async fn delete_analyzer(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock();
    let info = document.analyzer(uuid)?;
    document.remove_analyzer(uuid)?;
    data.push_undo(Command::AddAnalyzer(AnalyzerItemDto { id: uuid, info }));
    drop(document);
    Ok(HttpResponse::NoContent().finish())
}

/// Get a list of all analyzers of this model
#[utoipa::path(
    tag = "analyzer",
    responses((status = OK, description = "List of analyzers", body = Vec<AnalyzerItemDto>)),
)]
#[get("")]
pub async fn get_analyzers(data: web::Data<AppState>) -> HttpResponse {
    let analyzers_map = data.document.lock().analyzers();

    // Transform the HashMap into a list of DTOs containing the Uuid and the AnalyzerInfo
    let analyzers: Vec<AnalyzerItemDto> = analyzers_map
        .into_iter()
        .map(|(id, info)| {
            AnalyzerItemDto {
                id,
                info, // info already contains the correct gui_position logic internally
            }
        })
        .collect();

    HttpResponse::Ok().json(analyzers)
}

/// Update the analyzer config of an analyzer node
///
/// *Note*: This only updates the analyzer config. A GUI position is unchanged
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    request_body(content = AnalyzerType, description = "updated config of analyzer", content_type = "application/ron", example= "\"analyzer_type\""),
    responses(
        (status = NO_CONTENT, description = "Analyzer config successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Invalid RON or UUID not found", content_type="application/json")
    )
)]
#[patch("/{uuid}")]
pub async fn patch_analyzer(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: Ron<AnalyzerType>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let new = body.into_inner();
    let mut document = data.document.lock();

    let old = analyzer_mut_or_404(&mut document, uuid)?
        .analyzer_type()
        .clone();

    let command = Command::PatchAnalyzer(Box::new(PatchAnalyzer { id: uuid, old, new }));
    apply_and_push_undo(&data, document, command, true)
}

/// Update the GUI position of an analyzer
///
/// This endpoint is used by the frontend to update the 2D canvas coordinates
/// of an analyzer node after a drag-and-drop operation.
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    request_body(
        content = inline((f64, f64)),
        description = "New X and Y coordinates as a simple JSON array", 
        content_type = "application/json", 
        example = json!([150.5, -20.0])
    ),
    responses(
        (status = NO_CONTENT, description = "GUI position successfully updated"),
        (status = NOT_FOUND, body = ErrorResponse, description = "Analyzer UUID not found", content_type="application/json")
    )
)]
#[put("/{uuid}/gui_position")]
pub async fn put_analyzer_gui_position(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    gui_position: web::Json<(f64, f64)>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let new_pos = gui_position.into_inner();
    let mut document = data.document.lock();

    let old_pos = analyzer_mut_or_404(&mut document, uuid)?
        .gui_position()
        .map_or((0., 0.), |p| (p.x, p.y));

    let command = Command::RepositionAnalyzer(RepositionAnalyzer {
        id: uuid,
        old_pos,
        new_pos,
    });
    apply_and_push_undo(&data, document, command, true)
}

/// Set which pump scenarios an analyzer runs in
///
/// An analyzer produces one report per listed scenario, in the given order; an empty list is a
/// single passive run, which is what every analyzer did before scenarios existed.
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    request_body(content = Vec<Uuid>, description = "pump scenario UUIDs to run, in order", content_type = "application/json"),
    responses(
        (status = NO_CONTENT, description = "Pump scenario selection successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Analyzer UUID not found, or a listed pump scenario does not exist", content_type="application/json")
    )
)]
#[put("/{uuid}/pump_scenarios")]
pub async fn put_analyzer_pump_scenarios(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<Vec<Uuid>>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let new = body.into_inner();
    let mut document = data.document.lock();

    if let Some(missing) = new
        .iter()
        .find(|scenario_id| document.pump_scenario(**scenario_id).is_none())
    {
        return Err(BackEndErrorResponse::new(
            400,
            "Opossum",
            &format!("pump scenario {missing} does not exist"),
        ));
    }

    let old = analyzer_mut_or_404(&mut document, uuid)?
        .pump_scenarios()
        .to_vec();
    let command =
        Command::PatchAnalyzerPumpScenarios(PatchAnalyzerPumpScenarios { id: uuid, old, new });
    apply_and_push_undo(&data, document, command, true)
}

/// Get all available `SourcePort` nodes (UUID and Name) in the entire document recursively
#[utoipa::path(
    tag = "analyzer",
    responses(
        (status = OK, description = "List of all available SourcePort pairs in the document", body = Vec<SourcePortDto>),
        (status = INTERNAL_SERVER_ERROR, body = ErrorResponse, description = "Internal tree traversal error")
    )
)]
#[get("/available_sources")]
// The document lock is deliberately held for the whole read-only walk, as in the other lookup
// helpers - releasing it early would mean cloning the scenery for nothing.
#[allow(clippy::significant_drop_tightening)]
pub async fn get_available_sources(
    data: web::Data<AppState>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let document = data.document.lock();
    let collected_sources: Vec<SourcePortDto> = collect_source_ports(document.scenery())
        .into_iter()
        .map(|(uuid, name)| SourcePortDto { uuid, name })
        .collect();

    Ok(HttpResponse::Ok().json(collected_sources))
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_available_sources);
    cfg.service(get_analyzers);
    cfg.service(get_analyzer);
    cfg.service(post_analyzer);
    cfg.service(patch_analyzer);
    cfg.service(delete_analyzer);
    cfg.service(put_analyzer_gui_position);
    cfg.service(put_analyzer_pump_scenarios);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{app_state::AppState, document::undo_document};
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::prelude::EnergyConfig;

    #[actix_web::test]
    async fn set_pump_scenario_selection_and_undo() {
        let app_state = Data::new(AppState::default());
        let (analyzer_id, scenario_id) = {
            let mut document = app_state.document.lock();
            let analyzer_id = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
            let scenario_id = document.add_pump_scenario("full power");
            (analyzer_id, scenario_id)
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(put_analyzer_pump_scenarios)
                .service(undo_document),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/{analyzer_id}/pump_scenarios"))
            .set_json(vec![scenario_id])
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .pump_scenarios(),
            vec![scenario_id]
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .pump_scenarios()
                .is_empty()
        );
    }

    /// A selection naming a scenario that doesn't exist must be rejected outright - letting it
    /// through would only fail much later, at analysis time, with a less specific error.
    #[actix_web::test]
    async fn set_pump_scenario_selection_rejects_unknown_scenario() {
        let app_state = Data::new(AppState::default());
        let analyzer_id = {
            let mut document = app_state.document.lock();
            document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()))
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(put_analyzer_pump_scenarios),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/{analyzer_id}/pump_scenarios"))
            .set_json(vec![Uuid::new_v4()])
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(
            app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .pump_scenarios()
                .is_empty(),
            "a rejected selection must not be partially applied"
        );
    }
}
