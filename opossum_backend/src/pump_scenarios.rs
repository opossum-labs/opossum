//! Endpoints for the document-wide list of [`PumpScenario`]s - the operating points a model can be
//! analyzed in (see `opossum_core::gain::scenario` for the concept). Mirrors `analyzers.rs`'s
//! structure: list/get/create/delete plus field-level patches, all going through
//! [`Command::PatchPumpScenario`] for the parts that only ever replace the scenario wholesale
//! (rename, set a node's gain model or pump source).
use actix_web::{HttpRequest, HttpResponse, delete, get, post, put, web};
use opossum_core::{
    core_optics::{NodeAttr, NodeAttrExt},
    types::api_types::{
        ErrorResponse, NewPumpScenario, PumpScenarioItemDto, ScenarioAmplifierDto,
        SetScenarioGainModel, SetScenarioPumpSource,
    },
};
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::{
        apply_and_push_undo, collect_nodes, pump_scenario_mut_or_404, ron_or_json_response,
    },
    undo::{Command, PatchAnalyzerPumpScenarios, PatchPumpScenario},
};

/// Get a list of all pump scenarios of this model
#[utoipa::path(
    tag = "pump_scenario",
    responses((status = OK, description = "List of pump scenarios", body = Vec<PumpScenarioItemDto>)),
)]
#[get("")]
pub async fn get_pump_scenarios(data: web::Data<AppState>) -> HttpResponse {
    let scenarios: Vec<PumpScenarioItemDto> = data
        .document
        .lock()
        .pump_scenarios()
        .iter()
        .map(|(id, scenario)| PumpScenarioItemDto {
            id: *id,
            scenario: scenario.clone(),
        })
        .collect();
    HttpResponse::Ok().json(scenarios)
}

/// Get a pump scenario by UUID
///
/// The format is determined by the `Accept` header (`application/ron` or `application/json`).
/// Defaults to JSON.
#[utoipa::path(
    tag = "pump_scenario",
    params(("uuid" = Uuid, Path, description = "UUID of the pump scenario")),
    responses(
        (status = OK, description = "Pump scenario successfully retrieved", content((PumpScenarioItemDto = "application/json"), (PumpScenarioItemDto = "application/ron"))),
        (status = NOT_FOUND, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[get("/{uuid}")]
#[allow(clippy::future_not_send)]
pub async fn get_pump_scenario(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let scenario = data
        .document
        .lock()
        .pump_scenario(uuid)
        .cloned()
        .ok_or_else(BackEndErrorResponse::pump_scenario_not_found)?;
    ron_or_json_response(&req, &scenario)
}

/// Get every amplifier candidate, with what it does in the given pump scenario, names resolved
///
/// Lists every node in the document-wide amplifier-candidate set
/// (`OpmDocument::amplifier_nodes`), not just the ones actively configured in this scenario - an
/// unconfigured candidate is reported with the default
/// [`PumpConfig`](opossum_core::gain::PumpConfig), which neither pumps nor amplifies, so the
/// scenario editor can render one row per candidate and let it be turned on or off, rather than only
/// ever showing rows that already amplify. Walks the whole document recursively (nested groups
/// included), same traversal as `/api/nodes/amplifier_candidates`.
#[utoipa::path(
    tag = "pump_scenario",
    params(("uuid" = Uuid, Path, description = "UUID of the pump scenario")),
    responses(
        (status = OK, description = "List of every amplifier candidate, with its gain model in this scenario", body = Vec<ScenarioAmplifierDto>),
        (status = NOT_FOUND, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[get("/{uuid}/amplifiers")]
// Same reasoning as `get_amplifiers`: the lock covers the whole read-only tree walk on purpose.
#[allow(clippy::significant_drop_tightening)]
pub async fn get_pump_scenario_amplifiers(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock();
    let scenario = document
        .pump_scenario(uuid)
        .ok_or_else(BackEndErrorResponse::pump_scenario_not_found)?;
    let scenery = document.scenery();

    let amplifiers: Vec<ScenarioAmplifierDto> = collect_nodes(scenery, &|node_attr: &NodeAttr| {
        document.is_amplifier_node(node_attr.uuid()).then(|| {
            (
                node_attr.name().to_string(),
                node_attr.node_type().to_string(),
                scenario.config(node_attr.uuid()),
            )
        })
    })
    .into_iter()
    .map(|node| {
        let (name, node_type, config) = node.value;
        ScenarioAmplifierDto {
            uuid: node.uuid,
            name,
            node_type,
            group_id: node.group_id,
            group_name: scenery
                .with_group_node(node.group_id, |group| group.name().to_string())
                .unwrap_or_default(),
            config,
        }
    })
    .collect();

    Ok(HttpResponse::Ok().json(amplifiers))
}

/// Add a new, empty pump scenario to the model
#[utoipa::path(
    tag = "pump_scenario",
    request_body(content = NewPumpScenario, description = "name of the pump scenario to be created", content_type = "application/json", example = "{\"name\": \"Full power\"}"),
    responses((status = CREATED, body = Uuid, description = "Pump scenario successfully created"))
)]
#[post("")]
pub async fn post_pump_scenario(
    data: web::Data<AppState>,
    new_scenario: web::Json<NewPumpScenario>,
) -> HttpResponse {
    let mut document = data.document.lock();
    let id = document.add_pump_scenario(&new_scenario.name);
    // The scenario was just added, so it is always there to read back - this can't fail.
    if let Some(scenario) = document.pump_scenario(id) {
        data.push_undo(Command::RemovePumpScenario(PumpScenarioItemDto {
            id,
            scenario: scenario.clone(),
        }));
    }
    drop(document);
    HttpResponse::Created().json(id)
}

/// Delete a pump scenario
///
/// Also strips it from the selection of every analyzer that was running it - undoing the deletion
/// restores both the scenario and every affected analyzer's selection in one step.
#[utoipa::path(
    tag = "pump_scenario",
    params(("uuid" = Uuid, Path, description = "UUID of the pump scenario to delete")),
    responses(
        (status = NO_CONTENT, description = "Pump scenario deleted"),
        (status = NOT_FOUND, body = ErrorResponse, description = "Pump scenario not found")
    )
)]
#[delete("/{uuid}")]
pub async fn delete_pump_scenario(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock();
    let scenario = document
        .pump_scenario(uuid)
        .cloned()
        .ok_or_else(BackEndErrorResponse::pump_scenario_not_found)?;

    // Snapshot every analyzer's selection *before* removal, so the affected ones (those that had
    // `uuid` in their selection) can be told apart from the unaffected ones afterward.
    let selections_before: Vec<(Uuid, Vec<Uuid>)> = document
        .analyzers()
        .iter()
        .map(|(id, info)| (*id, info.pump_scenarios().to_vec()))
        .collect();

    // `remove_pump_scenario` strips `uuid` from every analyzer's selection as a side effect - see
    // its doc comment.
    document.remove_pump_scenario(uuid);

    let mut inverse = vec![Command::AddPumpScenario(PumpScenarioItemDto {
        id: uuid,
        scenario,
    })];
    for (analyzer_id, old_selection) in selections_before {
        if !old_selection.contains(&uuid) {
            continue;
        }
        let Ok(new_selection) = document
            .analyzer(analyzer_id)
            .map(|info| info.pump_scenarios().to_vec())
        else {
            continue;
        };
        inverse.push(Command::PatchAnalyzerPumpScenarios(
            PatchAnalyzerPumpScenarios {
                id: analyzer_id,
                old: new_selection,
                new: old_selection,
            },
        ));
    }
    drop(document);

    data.push_undo(
        Command::from_vec(inverse).expect("at least the AddPumpScenario entry is always present"),
    );
    Ok(HttpResponse::NoContent().finish())
}

/// Rename a pump scenario
#[utoipa::path(
    tag = "pump_scenario",
    params(("uuid" = Uuid, Path, description = "UUID of the pump scenario")),
    request_body(content = String, description = "new name", content_type = "application/json", example = "\"Half power\""),
    responses(
        (status = NO_CONTENT, description = "Pump scenario renamed"),
        (status = NOT_FOUND, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[put("/{uuid}/name")]
pub async fn put_pump_scenario_name(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    name: web::Json<String>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock();

    let old = pump_scenario_mut_or_404(&mut document, uuid)?.clone();
    let mut new = old.clone();
    new.set_name(&name);

    let command = Command::PatchPumpScenario(PatchPumpScenario { id: uuid, old, new });
    apply_and_push_undo(&data, document, command, true)
}

/// Set the gain model a node runs with within a pump scenario
///
/// Setting [`opossum_core::gain::GainModel::None`] takes the node out of the scenario again.
#[utoipa::path(
    tag = "pump_scenario",
    params(("uuid" = Uuid, Path, description = "UUID of the pump scenario")),
    request_body(content = SetScenarioGainModel, description = "node and gain model", content_type = "application/json"),
    responses(
        (status = NO_CONTENT, description = "Gain model set"),
        (status = NOT_FOUND, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[put("/{uuid}/gain_model")]
pub async fn put_pump_scenario_gain_model(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SetScenarioGainModel>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let SetScenarioGainModel {
        node_id,
        gain_model,
    } = body.into_inner();
    let mut document = data.document.lock();

    let old = pump_scenario_mut_or_404(&mut document, uuid)?.clone();
    let mut new = old.clone();
    new.set_gain_model(node_id, gain_model);

    let command = Command::PatchPumpScenario(PatchPumpScenario { id: uuid, old, new });
    apply_and_push_undo(&data, document, command, true)
}

/// Set how a node's medium is pumped within a pump scenario
///
/// The counterpart of [`put_pump_scenario_gain_model`] for the other half of the node's
/// [`PumpConfig`](opossum_core::gain::PumpConfig): setting
/// [`opossum_core::gain::PumpSource::None`] leaves the medium unpumped, and takes the node out of
/// the scenario if it does not amplify either.
#[utoipa::path(
    tag = "pump_scenario",
    params(("uuid" = Uuid, Path, description = "UUID of the pump scenario")),
    request_body(content = SetScenarioPumpSource, description = "node and pump source", content_type = "application/json"),
    responses(
        (status = NO_CONTENT, description = "Pump source set"),
        (status = NOT_FOUND, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[put("/{uuid}/pump_source")]
pub async fn put_pump_scenario_pump_source(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SetScenarioPumpSource>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let SetScenarioPumpSource { node_id, pump } = body.into_inner();
    let mut document = data.document.lock();

    let old = pump_scenario_mut_or_404(&mut document, uuid)?.clone();
    let mut new = old.clone();
    new.set_pump_source(node_id, pump);

    let command = Command::PatchPumpScenario(PatchPumpScenario { id: uuid, old, new });
    apply_and_push_undo(&data, document, command, true)
}

pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_pump_scenarios);
    cfg.service(get_pump_scenario);
    cfg.service(get_pump_scenario_amplifiers);
    cfg.service(post_pump_scenario);
    cfg.service(delete_pump_scenario);
    cfg.service(put_pump_scenario_name);
    cfg.service(put_pump_scenario_gain_model);
    cfg.service(put_pump_scenario_pump_source);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{app_state::AppState, document::undo_document};
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{
        gain::{ConstGain, ConstInversion, GainModel, PumpSource},
        nodes::Lens,
    };
    use utoipa_actix_web::scope;

    /// Mounts the pump-scenario endpoints under `/scenarios` and `undo_document` under `/undo`,
    /// exactly as `routes.rs` mounts them in production (just without the `/api` prefix) - the
    /// list/create endpoints sit at the scope root, which `http::Uri` cannot represent as an empty
    /// path, so they have to be tested behind a real scope rather than mounted bare.
    macro_rules! test_app {
        ($app_state:expr) => {
            test::init_service(
                App::new()
                    .app_data($app_state.clone())
                    .service(scope("/scenarios").configure(config))
                    .service(undo_document),
            )
            .await
        };
    }

    /// The list must resolve node names, report which group each candidate sits in, and include
    /// *every* candidate - configured in this scenario or not (with the default `PumpConfig` when
    /// not) - while never listing a node that isn't a candidate at all, even if some other scenario
    /// configured it.
    #[actix_web::test]
    async fn amplifiers_lists_every_candidate_with_its_config_in_this_scenario() {
        let app_state = Data::new(AppState::default());
        let (configured_id, unconfigured_id, non_candidate_id) = {
            let mut document = app_state.document.lock();
            let configured_id = document.scenery_mut().add_node(Lens::default()).unwrap();
            let unconfigured_id = document.scenery_mut().add_node(Lens::default()).unwrap();
            let non_candidate_id = document.scenery_mut().add_node(Lens::default()).unwrap();
            document.set_is_amplifier_node(configured_id, true);
            document.set_is_amplifier_node(unconfigured_id, true);
            (configured_id, unconfigured_id, non_candidate_id)
        };
        let scenario_id = add_scenario(&app_state, "full power").await;
        let other_scenario_id = add_scenario(&app_state, "half power").await;
        let gain = GainModel::Const(ConstGain::new(2.0).unwrap());
        {
            let mut document = app_state.document.lock();
            document
                .pump_scenario_mut(scenario_id)
                .unwrap()
                .set_gain_model(configured_id, gain);
            // A different scenario configuring the non-candidate must not make it appear here -
            // candidacy is document-wide, and shouldn't be possible after pruning anyway, but this
            // pins the invariant down.
            document
                .pump_scenario_mut(other_scenario_id)
                .unwrap()
                .set_gain_model(
                    non_candidate_id,
                    GainModel::Const(ConstGain::new(3.0).unwrap()),
                );
        }
        let app = test_app!(app_state);

        let req = test::TestRequest::get()
            .uri(&format!("/scenarios/{scenario_id}/amplifiers"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let amplifiers: Vec<ScenarioAmplifierDto> = test::read_body_json(resp).await;

        assert_eq!(
            amplifiers.len(),
            2,
            "must list exactly the two candidates, not the non-candidate: {amplifiers:?}"
        );
        let configured = amplifiers
            .iter()
            .find(|a| a.uuid == configured_id)
            .expect("the configured candidate must be listed");
        assert_eq!(configured.name, "lens");
        assert_eq!(configured.node_type, "lens");
        assert_eq!(configured.config.gain_model(), gain);
        let unconfigured = amplifiers
            .iter()
            .find(|a| a.uuid == unconfigured_id)
            .expect("the unconfigured candidate must be listed too");
        assert_eq!(
            unconfigured.config.gain_model(),
            GainModel::None,
            "a candidate this scenario hasn't configured must report GainModel::None, not be absent"
        );
        assert!(
            !amplifiers.iter().any(|a| a.uuid == non_candidate_id),
            "a non-candidate must never appear, even if some other scenario configured it"
        );
    }

    #[actix_web::test]
    async fn amplifiers_of_missing_scenario_is_404() {
        let app_state = Data::new(AppState::default());
        let app = test_app!(app_state);
        let req = test::TestRequest::get()
            .uri(&format!("/scenarios/{}/amplifiers", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// Adds a scenario with the given name to `data`'s document and returns its id.
    #[allow(clippy::future_not_send)]
    async fn add_scenario(data: &Data<AppState>, name: &str) -> Uuid {
        let app = test_app!(data);
        let req = test::TestRequest::post()
            .uri("/scenarios")
            .set_json(NewPumpScenario { name: name.into() })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        test::read_body_json(resp).await
    }

    #[actix_web::test]
    async fn create_list_and_get() {
        let app_state = Data::new(AppState::default());
        let id = add_scenario(&app_state, "full power").await;
        let app = test_app!(app_state);

        let req = test::TestRequest::get().uri("/scenarios").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let listed: Vec<PumpScenarioItemDto> = test::read_body_json(resp).await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, id);
        assert_eq!(listed[0].scenario.name(), "full power");

        let req = test::TestRequest::get()
            .uri(&format!("/scenarios/{id}"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let fetched: opossum_core::gain::PumpScenario = test::read_body_json(resp).await;
        assert_eq!(fetched.name(), "full power");
    }

    #[actix_web::test]
    async fn get_missing_scenario_is_404() {
        let app_state = Data::new(AppState::default());
        let app = test_app!(app_state);
        let req = test::TestRequest::get()
            .uri(&format!("/scenarios/{}", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn rename_and_undo_restores_old_name() {
        let app_state = Data::new(AppState::default());
        let id = add_scenario(&app_state, "full power").await;
        let app = test_app!(app_state);

        let req = test::TestRequest::put()
            .uri(&format!("/scenarios/{id}/name"))
            .set_json("half power")
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            app_state.document.lock().pump_scenario(id).unwrap().name(),
            "half power"
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            app_state.document.lock().pump_scenario(id).unwrap().name(),
            "full power"
        );
    }

    #[actix_web::test]
    async fn set_gain_model_and_undo_removes_it_again() {
        let app_state = Data::new(AppState::default());
        let scenario_id = add_scenario(&app_state, "full power").await;
        let node_id = {
            let mut document = app_state.document.lock();
            document.scenery_mut().add_node(Lens::default()).unwrap()
        };
        let app = test_app!(app_state);

        let gain = GainModel::Const(ConstGain::new(2.5).unwrap());
        let req = test::TestRequest::put()
            .uri(&format!("/scenarios/{scenario_id}/gain_model"))
            .set_json(SetScenarioGainModel {
                node_id,
                gain_model: gain,
            })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            app_state
                .document
                .lock()
                .pump_scenario(scenario_id)
                .unwrap()
                .gain_model(node_id),
            gain
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            app_state
                .document
                .lock()
                .pump_scenario(scenario_id)
                .unwrap()
                .gain_model(node_id),
            GainModel::None
        );
    }

    #[actix_web::test]
    async fn set_pump_source_and_undo_removes_it_again() {
        let app_state = Data::new(AppState::default());
        let scenario_id = add_scenario(&app_state, "full power").await;
        let node_id = {
            let mut document = app_state.document.lock();
            document.scenery_mut().add_node(Lens::default()).unwrap()
        };
        let app = test_app!(app_state);

        let pump = PumpSource::Const(
            ConstInversion::new(opossum_core::reciprocal_centimeter!(0.5)).unwrap(),
        );
        let req = test::TestRequest::put()
            .uri(&format!("/scenarios/{scenario_id}/pump_source"))
            .set_json(SetScenarioPumpSource { node_id, pump })
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            app_state
                .document
                .lock()
                .pump_scenario(scenario_id)
                .unwrap()
                .pump_source(node_id),
            pump
        );

        // The existing `PatchPumpScenario` command replaces the whole scenario, so it covers the
        // pump half without a command of its own - asserted rather than assumed.
        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            app_state
                .document
                .lock()
                .pump_scenario(scenario_id)
                .unwrap()
                .pump_source(node_id),
            PumpSource::None
        );
    }

    /// Switching the extraction model off must not throw away the pumping that was set up next to
    /// it - the two halves are edited through separate endpoints and must not overwrite each other.
    #[actix_web::test]
    async fn the_two_halves_are_set_independently() {
        let app_state = Data::new(AppState::default());
        let scenario_id = add_scenario(&app_state, "full power").await;
        let node_id = {
            let mut document = app_state.document.lock();
            document.scenery_mut().add_node(Lens::default()).unwrap()
        };
        let app = test_app!(app_state);

        let pump = PumpSource::Const(
            ConstInversion::new(opossum_core::reciprocal_centimeter!(0.5)).unwrap(),
        );
        let req = test::TestRequest::put()
            .uri(&format!("/scenarios/{scenario_id}/pump_source"))
            .set_json(SetScenarioPumpSource { node_id, pump })
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );
        let req = test::TestRequest::put()
            .uri(&format!("/scenarios/{scenario_id}/gain_model"))
            .set_json(SetScenarioGainModel {
                node_id,
                gain_model: GainModel::None,
            })
            .to_request();
        assert_eq!(
            app.call(req).await.unwrap().status(),
            StatusCode::NO_CONTENT
        );

        let document = app_state.document.lock();
        let scenario = document.pump_scenario(scenario_id).unwrap();
        assert_eq!(scenario.pump_source(node_id), pump);
        assert_eq!(scenario.amplifiers().count(), 1);
    }

    #[actix_web::test]
    async fn delete_and_undo_restores_the_scenario() {
        let app_state = Data::new(AppState::default());
        let id = add_scenario(&app_state, "full power").await;
        let app = test_app!(app_state);

        let req = test::TestRequest::delete()
            .uri(&format!("/scenarios/{id}"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(app_state.document.lock().pump_scenario(id).is_none());

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            app_state.document.lock().pump_scenario(id).unwrap().name(),
            "full power"
        );
    }

    /// Deleting a scenario an analyzer has selected must strip it from that selection too, and
    /// undoing the deletion must restore both the scenario itself and the selection - in one step.
    #[actix_web::test]
    async fn delete_selected_scenario_restores_analyzer_selection_on_undo() {
        let app_state = Data::new(AppState::default());
        let scenario_id = add_scenario(&app_state, "full power").await;
        let unrelated_id = add_scenario(&app_state, "cold").await;
        let analyzer_id = {
            let mut document = app_state.document.lock();
            let id = document.add_analyzer(opossum_core::prelude::AnalyzerType::Energy(
                opossum_core::prelude::EnergyConfig::default(),
            ));
            document
                .analyzer_mut(id)
                .unwrap()
                .set_pump_scenarios(vec![scenario_id, unrelated_id]);
            id
        };
        let app = test_app!(app_state);

        let req = test::TestRequest::delete()
            .uri(&format!("/scenarios/{scenario_id}"))
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
            vec![unrelated_id],
            "the deleted scenario must be gone from the analyzer's selection"
        );

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let document = app_state.document.lock();
        assert!(document.pump_scenario(scenario_id).is_some());
        assert_eq!(
            document.analyzer(analyzer_id).unwrap().pump_scenarios(),
            vec![scenario_id, unrelated_id],
            "undo must restore the analyzer's full selection, in its original order"
        );
        drop(document);
    }
}
