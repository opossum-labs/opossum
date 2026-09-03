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
    undo::{
        Command, PatchAnalyzer, PatchAnalyzerName, PatchAnalyzerPumpScenarios, RepositionAnalyzer,
    },
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
    default_wvl.map_or_else(EnergyDataBuilder::default, |wvl| {
        let ell =
            EnergyLaserLines::new(vec![(wvl, joule!(1.0))], nanometer!(0.1)).unwrap_or_default();
        EnergyDataBuilder::LaserLines(ell)
    })
}

/// Creates a default `RayDataBuilder` optionally configured with a custom wavelength.
pub fn create_default_ray_builder(default_wvl: Option<Length>) -> RayDataBuilder {
    default_wvl.map_or_else(RayDataBuilder::default, |wvl| {
        let mut rds = RayDataSource::default();
        if let Ok(lines) = LaserLines::new(vec![(wvl, 1.0)]) {
            rds.set_spectral_dist(SpecDistType::LaserLines(lines));
        }
        RayDataBuilder::from(rds)
    })
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

/// Set the user-assigned name of an analyzer
///
/// An empty string clears the name; the analyzer then shows its type label ("Energy", etc.)
/// in the GUI.
#[utoipa::path(
    tag = "analyzer",
    params(("uuid" = Uuid, Path, description = "UUID of the analyzer")),
    request_body(content = String, description = "new name (empty string to clear)", content_type = "application/json"),
    responses(
        (status = NO_CONTENT, description = "Analyzer name successfully updated"),
        (status = NOT_FOUND, body = ErrorResponse, description = "Analyzer UUID not found", content_type="application/json")
    )
)]
#[put("/{uuid}/name")]
pub async fn put_analyzer_name(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<String>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let new = body.into_inner();
    let mut document = data.document.lock();

    let old = analyzer_mut_or_404(&mut document, uuid)?.name().to_string();
    let command = Command::PatchAnalyzerName(PatchAnalyzerName { id: uuid, old, new });
    apply_and_push_undo(&data, document, command, true)
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
    cfg.service(put_analyzer_name);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        app_state::AppState,
        document::{redo_document, undo_document},
    };
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
    #[actix_web::test]
    async fn test_get_analyzers_and_get_single_analyzer() {
        let app_state = Data::new(AppState::default());
        let analyzer_id = {
            let mut document = app_state.document.lock();
            document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()))
        };

        // Mount services under a scope so that the empty path #[get("")] resolves properly
        let app = test::init_service(
            App::new().app_data(app_state.clone()).service(
                web::scope("/analyzers")
                    .service(get_analyzers)
                    .service(get_analyzer),
            ),
        )
        .await;

        // 1. Test get_analyzers list (matches scope with empty subpath)
        let req = test::TestRequest::get().uri("/analyzers").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let list: Vec<AnalyzerItemDto> = test::read_body_json(resp).await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, analyzer_id);

        // 2. Test get_analyzer for existing UUID (matches /{uuid})
        let req = test::TestRequest::get()
            .uri(&format!("/analyzers/{analyzer_id}"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let info: AnalyzerInfo = test::read_body_json(resp).await;
        assert!(matches!(info.analyzer_type(), AnalyzerType::Energy(_)));

        // 3. Test get_analyzer with unknown UUID returns 404
        let req = test::TestRequest::get()
            .uri(&format!("/analyzers/{}", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_put_analyzer_name_and_undo() {
        let app_state = Data::new(AppState::default());
        let analyzer_id = {
            let mut document = app_state.document.lock();
            document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()))
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(put_analyzer_name)
                .service(undo_document),
        )
        .await;

        // Set custom name
        let req = test::TestRequest::put()
            .uri(&format!("/{analyzer_id}/name"))
            .set_json("Diagnostic Energy Analyzer")
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .name(),
            "Diagnostic Energy Analyzer"
        );

        // Undo reverts the name
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .name(),
            ""
        );

        // Renaming an unknown UUID returns 404
        let req = test::TestRequest::put()
            .uri(&format!("/{}/name", Uuid::new_v4()))
            .set_json("Unknown")
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn test_put_analyzer_gui_position_and_undo() {
        let app_state = Data::new(AppState::default());
        let analyzer_id = {
            let mut document = app_state.document.lock();
            document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()))
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(put_analyzer_gui_position)
                .service(undo_document),
        )
        .await;

        // Update coordinates
        let req = test::TestRequest::put()
            .uri(&format!("/{analyzer_id}/gui_position"))
            .set_json((250.0, -100.0))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        let pos = app_state
            .document
            .lock()
            .analyzer(analyzer_id)
            .unwrap()
            .gui_position()
            .unwrap();
        assert_eq!((pos.x, pos.y), (250.0, -100.0));

        // Undo reverts position
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Repositioning an unknown UUID returns 404
        let req = test::TestRequest::put()
            .uri(&format!("/{}/gui_position", Uuid::new_v4()))
            .set_json((10.0, 10.0))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
    #[actix_web::test]
    async fn test_post_analyzer_and_undo_redo() {
        let app_state = Data::new(AppState::default());

        // Mount endpoints under a scope to match root-level empty paths correctly
        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(
                    web::scope("/analyzers")
                        .service(post_analyzer)
                        .service(get_analyzer),
                )
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // 1. Create a NewAnalyzerInfo payload using the provided constructor
        let new_analyzer_payload = NewAnalyzerInfo::new(
            AnalyzerType::Energy(EnergyConfig::default()),
            (120.0, -45.0),
            Some(nanometer!(1064.0)),
        );

        let req = test::TestRequest::post()
            .uri("/analyzers")
            .set_json(&new_analyzer_payload)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Extract the generated UUID from the JSON response
        let created_id: Uuid = test::read_body_json(resp).await;

        // 2. Verify the analyzer was created with the expected properties in state
        {
            let document = app_state.document.lock();
            let analyzer_info = document.analyzer(created_id).expect("analyzer must exist");
            assert_eq!(
                analyzer_info.gui_position().map(|p| (p.x, p.y)),
                Some((120.0, -45.0))
            );
            assert!(matches!(
                analyzer_info.analyzer_type(),
                AnalyzerType::Energy(_)
            ));
        }

        // 3. Undo removes the freshly created analyzer
        let req_undo = test::TestRequest::post().uri("/undo").to_request();
        let resp_undo = app.call(req_undo).await.unwrap();
        assert_eq!(resp_undo.status(), StatusCode::OK);
        assert!(app_state.document.lock().analyzer(created_id).is_err());

        // 4. Redo restores the analyzer under the identical UUID
        let req_redo = test::TestRequest::post().uri("/redo").to_request();
        let resp_redo = app.call(req_redo).await.unwrap();
        assert_eq!(resp_redo.status(), StatusCode::OK);
        assert!(app_state.document.lock().analyzer(created_id).is_ok());
    }
    #[actix_web::test]
    async fn test_patch_analyzer_and_undo_redo() {
        let app_state = Data::new(AppState::default());
        let analyzer_id = {
            let mut document = app_state.document.lock();
            document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()))
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(web::scope("/analyzers").service(patch_analyzer))
                .service(undo_document)
                .service(redo_document),
        )
        .await;

        // 1. Prepare a modified analyzer configuration
        let mut modified_config = EnergyConfig::default();
        let source_port_id = Uuid::new_v4();
        modified_config.map_source(source_port_id, create_default_energy_builder(None));
        let updated_analyzer_type = AnalyzerType::Energy(modified_config);

        // Serialize the payload into RON format
        let ron_payload = ron::to_string(&updated_analyzer_type)
            .expect("failed to serialize AnalyzerType to RON");

        // 2. Send PATCH request with application/ron content type
        let req = test::TestRequest::patch()
            .uri(&format!("/analyzers/{analyzer_id}"))
            .insert_header((actix_web::http::header::CONTENT_TYPE, "application/ron"))
            .set_payload(ron_payload.clone())
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // Verify analyzer configuration was successfully updated in document state
        assert_eq!(
            app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .analyzer_type(),
            &updated_analyzer_type
        );

        // 3. Undo restores original configuration
        let req_undo = test::TestRequest::post().uri("/undo").to_request();
        let resp_undo = app.call(req_undo).await.unwrap();
        assert_eq!(resp_undo.status(), StatusCode::OK);
        assert_eq!(
            app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .analyzer_type(),
            &AnalyzerType::Energy(EnergyConfig::default())
        );

        // 4. Redo reapplies the patched configuration
        let req_redo = test::TestRequest::post().uri("/redo").to_request();
        let resp_redo = app.call(req_redo).await.unwrap();
        assert_eq!(resp_redo.status(), StatusCode::OK);
        assert_eq!(
            app_state
                .document
                .lock()
                .analyzer(analyzer_id)
                .unwrap()
                .analyzer_type(),
            &updated_analyzer_type
        );

        // 5. Error case: patching an unknown UUID must return 404
        let req_not_found = test::TestRequest::patch()
            .uri(&format!("/analyzers/{}", Uuid::new_v4()))
            .insert_header((actix_web::http::header::CONTENT_TYPE, "application/ron"))
            .set_payload(ron_payload)
            .to_request();
        let resp_not_found = app.call(req_not_found).await.unwrap();
        assert_eq!(resp_not_found.status(), StatusCode::NOT_FOUND);

        // 6. Error case: malformed RON payload must be rejected
        let req_bad_ron = test::TestRequest::patch()
            .uri(&format!("/analyzers/{analyzer_id}"))
            .insert_header((actix_web::http::header::CONTENT_TYPE, "application/ron"))
            .set_payload("invalid ron content (())")
            .to_request();
        let resp_bad_ron = app.call(req_bad_ron).await.unwrap();
        assert!(!resp_bad_ron.status().is_success());
    }
    #[actix_web::test]
    async fn test_delete_analyzer_and_undo() {
        let app_state = Data::new(AppState::default());
        let analyzer_id = {
            let mut document = app_state.document.lock();
            document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()))
        };

        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(delete_analyzer)
                .service(undo_document),
        )
        .await;

        // Delete analyzer
        let req = test::TestRequest::delete()
            .uri(&format!("/{analyzer_id}"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(app_state.document.lock().analyzer(analyzer_id).is_err());

        // Undo restores it
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(app_state.document.lock().analyzer(analyzer_id).is_ok());

        // Deleting unknown UUID fails
        let req = test::TestRequest::delete()
            .uri(&format!("/{}", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert!(!resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_get_available_sources() {
        let app_state = Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(get_available_sources),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/available_sources")
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let sources: Vec<SourcePortDto> = test::read_body_json(resp).await;
        assert!(sources.is_empty());
    }

    #[actix_web::test]
    async fn test_builder_constructors() {
        // Test Energy builders with and without explicit wavelength
        let default_energy = create_default_energy_builder(None);
        assert!(matches!(default_energy, EnergyDataBuilder::LaserLines(_)));

        let custom_energy = create_default_energy_builder(Some(nanometer!(1064.0)));
        assert!(matches!(custom_energy, EnergyDataBuilder::LaserLines(_)));

        // Test Ray builders with and without explicit wavelength
        let default_ray = create_default_ray_builder(None);
        let custom_ray = create_default_ray_builder(Some(nanometer!(1064.0)));
        let _ = (default_ray, custom_ray);
    }
}
