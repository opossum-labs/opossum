//! Pump scenario api calls.

use crate::HTTP_API_CLIENT;
use opossum_core::{
    gain::{GainModel, PumpScenario},
    types::api_types::{AmplifierDto, NewPumpScenario, PumpScenarioItemDto, SetScenarioGainModel},
};
use uuid::Uuid;

/// Get all pump scenarios of the document.
///
/// # Errors
///
/// This function will return an error if the request fails or the response cannot be deserialized.
pub async fn get_pump_scenarios() -> Result<Vec<PumpScenarioItemDto>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<PumpScenarioItemDto>>("/api/pump_scenarios")
        .await
}

/// Get a single pump scenario by its UUID.
///
/// # Errors
///
/// This function will return an error if the request fails, the UUID is not found, or the
/// response cannot be deserialized.
pub async fn get_pump_scenario(uuid: Uuid) -> Result<PumpScenario, String> {
    HTTP_API_CLIENT()
        .get::<PumpScenario>(&format!("/api/pump_scenarios/{uuid}"))
        .await
}

/// Get every node the given pump scenario amplifies, with names resolved.
///
/// # Errors
///
/// This function will return an error if the request fails, the UUID is not found, or the
/// response cannot be deserialized.
pub async fn get_pump_scenario_amplifiers(uuid: Uuid) -> Result<Vec<AmplifierDto>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<AmplifierDto>>(&format!("/api/pump_scenarios/{uuid}/amplifiers"))
        .await
}

/// Add a new, empty pump scenario with the given name.
///
/// # Errors
///
/// This function will return an error if the request fails or the response cannot be deserialized.
pub async fn post_pump_scenario(name: &str) -> Result<Uuid, String> {
    HTTP_API_CLIENT()
        .post::<NewPumpScenario, Uuid>("/api/pump_scenarios", NewPumpScenario { name: name.into() })
        .await
}

/// Delete a pump scenario.
///
/// Also strips it from the selection of every analyzer that was running it - see the endpoint's
/// own doc comment.
///
/// # Errors
///
/// This function will return an error if the request fails or the UUID is not found.
pub async fn delete_pump_scenario(uuid: Uuid) -> Result<(), String> {
    HTTP_API_CLIENT()
        .delete_no_content(&format!("/api/pump_scenarios/{uuid}"))
        .await
}

/// Rename a pump scenario.
///
/// # Errors
///
/// This function will return an error if the request fails or the UUID is not found.
pub async fn put_pump_scenario_name(uuid: Uuid, name: String) -> Result<(), String> {
    HTTP_API_CLIENT()
        .put_receive_no_content(&format!("/api/pump_scenarios/{uuid}/name"), name)
        .await
}

/// Set the gain model a node runs with within a pump scenario.
///
/// [`GainModel::None`] takes the node out of the scenario again.
///
/// # Errors
///
/// This function will return an error if the request fails or the UUID is not found.
pub async fn put_pump_scenario_gain_model(
    scenario_id: Uuid,
    node_id: Uuid,
    gain_model: GainModel,
) -> Result<(), String> {
    HTTP_API_CLIENT()
        .put_receive_no_content(
            &format!("/api/pump_scenarios/{scenario_id}/gain_model"),
            SetScenarioGainModel {
                node_id,
                gain_model,
            },
        )
        .await
}

// A client wrapper for `PUT /api/analyzers/{uuid}/pump_scenarios` (setting an analyzer's scenario
// selection) belongs here once the analyzer editor grows the widget that calls it - the endpoint
// itself already exists and is tested on the backend (`opossum_backend::analyzers`).
