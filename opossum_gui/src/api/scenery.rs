//! Scenery api calls

use crate::HTTP_API_CLIENT;
use opossum_core::{opm_document::AnalyzerInfo, types::api_types::NewAnalyzerInfo};
use uuid::Uuid;

/// Send a request to delete the current scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the scenery is not valid)
pub async fn delete_scenery() -> Result<String, String> {
    HTTP_API_CLIENT()
        .delete::<String, String>("/api/scenery/", String::new())
        .await
}
/// Send a request to add an analyzer to the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`AnalyzerType`] cannot be serialized.
pub async fn post_add_analyzer(new_analyzer_info: NewAnalyzerInfo) -> Result<Uuid, String> {
    HTTP_API_CLIENT()
        .post::<NewAnalyzerInfo, Uuid>("/api/scenery/analyzers", new_analyzer_info)
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
        .get::<Vec<AnalyzerInfo>>("/api/scenery/analyzers")
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
        .delete::<String, Uuid>(&format!("/api/scenery/analyzers/{id}"), String::new())
        .await
}
/// Send request to receive the `OPM` file representation (as string) of the scenery.
/// This function is used to while saving a model file to disk.
///
/// # Errors
///
/// This function will return an error if .
pub async fn get_opm_file() -> Result<String, String> {
    HTTP_API_CLIENT().get_raw("/api/scenery/opmfile").await
}
/// Send request to load a scenery from an `OPM` file (string).
///
/// # Errors
///
/// This function will return an error if
/// - the `OPM` file cannot be parsed
/// - the scenery cannot be constructed from the file data.
pub async fn post_opm_file(opm_string: String) -> Result<String, String> {
    HTTP_API_CLIENT()
        .post_string("/api/scenery/opmfile", opm_string)
        .await
}

pub async fn get_scenery_uuid() -> Result<Uuid, String> {
    HTTP_API_CLIENT().get("/api/scenery/scenery_uuid").await
}
