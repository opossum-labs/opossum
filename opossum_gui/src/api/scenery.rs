//! Scenery api calls

use super::http_client::HTTPClient;
use opossum_backend::scenery::NewAnalyzerInfo;
use uuid::Uuid;

/// Send a request to delete the current scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
pub async fn delete_scenery(client: &HTTPClient) -> Result<String, String> {
    client
        .delete::<String, String>("/api/scenery/", String::new())
        .await
}
/// Send a request to add an analyzer to the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`AnalyzerType`] cannot be serialized.
pub async fn post_add_analyzer(
    client: &HTTPClient,
    new_analyzer_info: NewAnalyzerInfo,
) -> Result<Uuid, String> {
    client
        .post::<NewAnalyzerInfo, Uuid>("/api/scenery/analyzers", new_analyzer_info)
        .await
}
/// Send request to delete an analyzer with the given id.
///
/// # Errors
///
/// This function will return an error if
/// - the Analyzer with the given id was not found.
pub async fn delete_analyzer(client: &HTTPClient, id: Uuid) -> Result<String, String> {
    client
        .delete::<String, String>(&format!("/api/scenery/analyzers/{id}"), String::new())
        .await
}
/// Send request to receive the `OPM` file representation (as string) of the scenery.
/// This function is used to while saving a model file to disk.
///
/// # Errors
///
/// This function will return an error if .
pub async fn get_opm_file(client: &HTTPClient) -> Result<String, String> {
    client.get_raw("/api/scenery/opmfile").await
}
/// Send request to load a scenery from an `OPM` file (string).
///
/// # Errors
///
/// This function will return an error if
/// - the `OPM` file cannot be parsed
/// - the scenery cannot be constructed from the file data.
pub async fn post_opm_file(client: &HTTPClient, opm_string: String) -> Result<String, String> {
    client.post_string("/api/scenery/opmfile", opm_string).await
}
