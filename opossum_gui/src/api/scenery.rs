//! Scenery api calls

use opossum_backend::scenery::NewAnalyzerInfo;
use uuid::Uuid;

use super::http_client::HTTPClient;

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
// pub async fn get_analyzers(&self) -> Result<Vec<AnalyzerType>, String> {
//     self.get::<Vec<AnalyzerType>>("/api/scenery/analyzers")
//         .await
// }

/// Send a request to add an analyzer to the scenery.
///
/// # Errors
///
/// This function will return an error if
/// - the provided [`AnalyzerType`] cannot be serialized
pub async fn post_add_analyzer(
    client: &HTTPClient,
    new_analyzer_info: NewAnalyzerInfo,
) -> Result<Uuid, String> {
    client
        .post::<NewAnalyzerInfo, Uuid>("/api/scenery/analyzers", new_analyzer_info)
        .await
}
pub async fn delete_analyzer(client: &HTTPClient, id: Uuid) -> Result<String, String> {
    client
        .delete::<String, String>(&format!("/api/scenery/analyzers/{}", id), String::new())
        .await
}
