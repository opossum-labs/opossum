//! Endpoints for the document-wide amplifier-candidate set (`OpmDocument::amplifier_nodes`) - the
//! hardware-side half of the Hardware/Betriebspunkt split (see `opossum_core::gain::scenario`).
//! Candidacy here does not depend on any [`PumpScenario`]; a node's actual gain model in one
//! operating point is configured separately, per scenario, in `crate::pump_scenarios`.
use std::collections::HashSet;

use actix_web::{HttpResponse, get, put, web};
use opossum_core::{gain::PumpScenario, types::api_types::ErrorResponse};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    error::BackEndErrorResponse,
    helper_functions::parent_group_id_or_self,
    undo::{Command, PatchAmplifierNodes, PatchPumpScenario},
};

/// Get the whole document's amplifier-candidate set
///
/// A plain list of node uuids - the GUI only needs this to know which nodes to treat as candidates
/// on the canvas and in every scenario editor; names are resolved separately (per scenario) via
/// `GET /api/pump_scenarios/{uuid}/amplifiers`.
#[utoipa::path(
    tag = "node",
    responses((status = OK, description = "List of amplifier candidate node uuids", body = Vec<Uuid>)),
)]
#[get("/amplifier_candidates")]
pub async fn get_amplifier_candidates(data: web::Data<AppState>) -> HttpResponse {
    let candidates: Vec<Uuid> = data
        .document
        .lock()
        .amplifier_nodes()
        .iter()
        .copied()
        .collect();
    HttpResponse::Ok().json(candidates)
}

/// Mark or unmark a node as an amplifier candidate
///
/// Whether a node **is** an amplifier is a hardware fact, independent of any pump scenario - this
/// is what the context menu's "As amplifier"/"As passive optic" toggle edits. Unmarking a node also
/// strips it from every scenario's gain-model map (see
/// [`OpmDocument::set_is_amplifier_node`](opossum_core::opm_document::OpmDocument::set_is_amplifier_node)),
/// so undo restores the candidacy and every affected scenario's configuration in one step.
#[utoipa::path(
    tag = "node",
    params(("uuid" = Uuid, Path, description = "UUID of the optical node")),
    request_body(content = bool, description = "whether the node is an amplifier candidate from now on", content_type = "application/json", example = "true"),
    responses(
        (status = NO_CONTENT, description = "Candidacy updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type = "application/json")
    )
)]
#[put("/{uuid}/is_amplifier")]
pub async fn put_node_is_amplifier(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    is_amplifier: web::Json<bool>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let is_amplifier = is_amplifier.into_inner();
    let mut document = data.document.lock();
    parent_group_id_or_self(document.scenery(), uuid)?;

    // Snapshot every scenario's state *before* the toggle, so the ones the toggle actually prunes
    // (turning candidacy off) can be told apart from the untouched ones afterward - same shape
    // `delete_pump_scenario` already uses for its "which analyzers lost the selection" step.
    let scenarios_before: Vec<(Uuid, PumpScenario)> = document
        .pump_scenarios()
        .iter()
        .map(|(id, scenario)| (*id, scenario.clone()))
        .collect();
    let candidates_before: HashSet<Uuid> = document.amplifier_nodes().clone();

    document.set_is_amplifier_node(uuid, is_amplifier);

    let mut inverse = vec![Command::PatchAmplifierNodes(PatchAmplifierNodes {
        old: document.amplifier_nodes().clone(),
        new: candidates_before,
    })];
    for (scenario_id, old_scenario) in scenarios_before {
        let Some(new_scenario) = document.pump_scenario(scenario_id) else {
            continue;
        };
        if *new_scenario != old_scenario {
            inverse.push(Command::PatchPumpScenario(PatchPumpScenario {
                id: scenario_id,
                old: new_scenario.clone(),
                new: old_scenario,
            }));
        }
    }
    drop(document);

    data.push_undo(
        Command::from_vec(inverse)
            .expect("at least the PatchAmplifierNodes entry is always present"),
    );
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{app_state::AppState, document::undo_document};
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{
        gain::{ConstGain, GainModel},
        nodes::Lens,
        types::api_types::UndoRedoResponse,
    };

    /// Mounts the amplifier-candidate endpoints directly (they sit at the scope root of
    /// `/api/nodes` in production) and `undo_document` under `/undo`, mirroring the pattern
    /// `pump_scenarios.rs`'s tests use.
    macro_rules! test_app {
        ($app_state:expr) => {
            test::init_service(
                App::new()
                    .app_data($app_state.clone())
                    .service(get_amplifier_candidates)
                    .service(put_node_is_amplifier)
                    .service(undo_document),
            )
            .await
        };
    }

    #[actix_web::test]
    async fn toggle_on_and_off_with_undo() {
        let app_state = Data::new(AppState::default());
        let node_id = {
            let mut document = app_state.document.lock();
            document.scenery_mut().add_node(Lens::default()).unwrap()
        };
        let app = test_app!(app_state);

        let req = test::TestRequest::put()
            .uri(&format!("/{node_id}/is_amplifier"))
            .set_json(true)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(app_state.document.lock().is_amplifier_node(node_id));

        let req = test::TestRequest::get()
            .uri("/amplifier_candidates")
            .to_request();
        let resp = app.call(req).await.unwrap();
        let candidates: Vec<Uuid> = test::read_body_json(resp).await;
        assert_eq!(candidates, vec![node_id]);

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(!app_state.document.lock().is_amplifier_node(node_id));
    }

    #[actix_web::test]
    async fn toggling_a_missing_node_is_400() {
        let app_state = Data::new(AppState::default());
        let app = test_app!(app_state);

        let req = test::TestRequest::put()
            .uri(&format!("/{}/is_amplifier", Uuid::new_v4()))
            .set_json(true)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// Unmarking a node configured in two scenarios must prune both, and undo must restore the
    /// candidacy *and* both scenarios' configured values in one step.
    #[actix_web::test]
    async fn unmarking_prunes_every_scenario_and_undo_restores_all_in_one_step() {
        let app_state = Data::new(AppState::default());
        let gain = GainModel::Const(ConstGain::new(2.5).unwrap());
        let (node_id, full_power, half_power) = {
            let mut document = app_state.document.lock();
            let node_id = document.scenery_mut().add_node(Lens::default()).unwrap();
            document.set_is_amplifier_node(node_id, true);
            let full_power = document.add_pump_scenario("full power");
            let half_power = document.add_pump_scenario("half power");
            for id in [full_power, half_power] {
                document
                    .pump_scenario_mut(id)
                    .unwrap()
                    .set_gain_model(node_id, gain);
            }
            (node_id, full_power, half_power)
        };
        let app = test_app!(app_state);

        let req = test::TestRequest::put()
            .uri(&format!("/{node_id}/is_amplifier"))
            .set_json(false)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        {
            let document = app_state.document.lock();
            assert!(!document.is_amplifier_node(node_id));
            for id in [full_power, half_power] {
                assert_eq!(
                    document.pump_scenario(id).unwrap().gain_model(node_id),
                    GainModel::None,
                    "unmarking must prune every scenario, not just one"
                );
            }
        }

        let req = test::TestRequest::post().uri("/undo").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: UndoRedoResponse = test::read_body_json(resp).await;
        assert!(
            body.changes.iter().any(|c| matches!(
                c,
                opossum_core::types::api_types::DocumentChange::AmplifierNodesChanged
            )),
            "undo must report the candidate set changed: {:?}",
            body.changes
        );

        let document = app_state.document.lock();
        assert!(
            document.is_amplifier_node(node_id),
            "undo must restore the candidacy"
        );
        for id in [full_power, half_power] {
            assert_eq!(
                document.pump_scenario(id).unwrap().gain_model(node_id),
                gain,
                "undo must restore every scenario's configured value in the same step"
            );
        }
    }

    /// Marking a node that no scenario has ever heard of must not touch any scenario at all.
    #[actix_web::test]
    async fn marking_a_node_touches_no_scenario() {
        let app_state = Data::new(AppState::default());
        let (node_id, scenario_id) = {
            let mut document = app_state.document.lock();
            let node_id = document.scenery_mut().add_node(Lens::default()).unwrap();
            let scenario_id = document.add_pump_scenario("full power");
            (node_id, scenario_id)
        };
        let app = test_app!(app_state);

        let req = test::TestRequest::put()
            .uri(&format!("/{node_id}/is_amplifier"))
            .set_json(true)
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
            GainModel::None
        );
    }
}
