//! Scenery api calls

use crate::HTTP_API_CLIENT;
use opossum_core::{
    opm_document::AnalyzerInfo, prelude::AnalyzerType, types::api_types::NewAnalyzerInfo,
};
use uuid::Uuid;

/// Send a request to add an analyzer to the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`AnalyzerType`] cannot be serialized.
pub async fn post_add_analyzer(new_analyzer_info: NewAnalyzerInfo) -> Result<Uuid, String> {
    HTTP_API_CLIENT()
        .post::<NewAnalyzerInfo, Uuid>("/api/analyzers", new_analyzer_info)
        .await
}
/// Get all available analyzers.
///
/// Return a list of all available analyzers.
///
/// # Errors
///
/// This function will return an error if
/// - the returned data cannot be parsed (deserialized) into the correct data type.
pub async fn get_analyzers() -> Result<Vec<AnalyzerInfo>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<AnalyzerInfo>>("/api/analyzers")
        .await
}
/// Send request to delete an analyzer with the given id.
///
/// # Errors
///
/// This function will return an error if
/// - the Analyzer with the given id was not found.
pub async fn delete_analyzer(id: Uuid) -> Result<Uuid, String> {
    HTTP_API_CLIENT()
        .delete::<String, Uuid>(&format!("/api/analyzers/{id}"), String::new())
        .await
}
/// Update the analyzer configuration of an analyzer node.
/// This function sends a POST request to the server to update the analyzer type
/// associated with a specific node identified by its UUID. The server endpoint is:
/// `/api/scenery/analyzer/{node_id}`.
///
/// # Parameters
/// - `client`: An instance of [`HTTPClient`] used to send the request.
/// - `node_id`: The unique identifier of the node whose analyzer configuration is to be updated.
/// - `analyzer_type`: The new [`AnalyzerType`] to apply to the node.
/// # Returns
/// A [`Result`] containing:
/// - `Ok(String)`: The server's response if the update is successful.
/// - `Err(String)`: An error message returned from the server or the HTTP client.
/// # Errors
/// This function returns an error if:
/// - The HTTP request fails to reach the server (e.g., network issues).
/// - The server responds with an error status code (e.g., 4xx or 5xx).
/// - Serialization of the [`AnalyzerType`] payload fails before sending.
pub async fn update_analyzer_config_ron(
    node_id: Uuid,
    analyzer_type: AnalyzerType,
) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post_ron::<AnalyzerType, String>(&format!("/api/analyzers/{node_id}"), analyzer_type)
        .await
}
pub async fn update_analyzer_position(node_id: Uuid, position: (f64, f64)) -> Result<(), String> {
    HTTP_API_CLIENT()
        .put::<(f64, f64), ()>(&format!("/api/analyzers/{node_id}/gui_position"), position)
        .await
}
/// Get the information about an analyzer node.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`Uuid`] cannot be serialized or found
/// - the properties cannot be deserialized into the [`AnalyzerInfo`] struct
pub async fn get_analyzer(uuid: Uuid) -> Result<AnalyzerInfo, String> {
    HTTP_API_CLIENT()
        .get_ron::<AnalyzerInfo>(&format!("/api/analyzers/{uuid}"))
        .await
}
