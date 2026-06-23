//! Scenery api calls

use crate::HTTP_API_CLIENT;
use dioxus::html::geometry::euclid::default::Point2D;
use opossum_core::{
    opm_document::AnalyzerInfo,
    prelude::AnalyzerType,
    types::api_types::{AnalyzerItemDto, NewAnalyzerInfo, SourcePortDto},
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
pub async fn get_analyzers() -> Result<Vec<AnalyzerItemDto>, String> {
    // Update the return type and the generic type parsing the HTTP response
    HTTP_API_CLIENT()
        .get::<Vec<AnalyzerItemDto>>("/api/analyzers")
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
        .delete_no_content(&format!("/api/analyzers/{id}"))
        .await
        .map(|()| id)
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
) -> Result<(), String> {
    HTTP_API_CLIENT()
        .patch_ron::<AnalyzerType>(&format!("/api/analyzers/{node_id}"), analyzer_type)
        .await
}
pub async fn update_analyzer_position(
    node_id: Uuid,
    gui_position: Point2D<f64>,
) -> Result<(), String> {
    let position = (gui_position.x, gui_position.y);
    HTTP_API_CLIENT()
        .put_receive_no_content::<(f64, f64)>(
            &format!("/api/analyzers/{node_id}/gui_position"),
            position,
        )
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
/// Get a list of `SourcePort` Nodes in the scenery.
///
/// This returns a list with a Uuid <-> node name mapping.
pub async fn get_available_sources() -> Result<Vec<SourcePortDto>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<SourcePortDto>>("/api/analyzers/available_sources")
        .await
}
