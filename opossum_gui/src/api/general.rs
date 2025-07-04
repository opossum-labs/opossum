//! General api calls

use super::http_client::HTTPClient;
use opossum_backend::{
    general::{NodeType, VersionInfo},
    AnalyzerType,
};

/// Send a request to check if the bace url is reachable and corresponds to the opossum backend.
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the base url is not reachable)
/// - the response cannot be deserialized into a string
#[allow(dead_code)]
pub async fn get_api_welcome(client: &HTTPClient) -> Result<String, String> {
    client.get::<String>("/api/").await
}

/// Send reqeust to get the version of the opossum backend and the opossum library.
///
/// # Errors
///
/// This function will return an error if
/// - the `VersionInfo` struct cannot be deserialized
pub async fn get_version(client: &HTTPClient) -> Result<VersionInfo, String> {
    client.get::<VersionInfo>("/api/version").await
}

/// Send a request to get all available node types.
///
/// # Errors
///
/// This function will return an error if
/// - the response cannot be deserialized into a vector of [`NodeType`] structs.
pub async fn get_node_types(client: &HTTPClient) -> Result<Vec<NodeType>, String> {
    client.get::<Vec<NodeType>>("/api/node_types").await
}

/// Send a request to get all available anaylzer types.
///
/// # Errors
///
/// This function will return an error if
/// - the response cannot be deserialized into a vector of [`AnalyzerType`] structs.
pub async fn get_analyzer_types(client: &HTTPClient) -> Result<Vec<AnalyzerType>, String> {
    client.get::<Vec<AnalyzerType>>("/api/analyzer_types").await
}

/// Send a request to shutdown the backend server.
///
/// This function shuts down the backend server. No further communication is possible after this call.
#[allow(dead_code)]
pub async fn post_terminate(client: &HTTPClient) {
    let _ = client.client().post("/api/terminate").send().await;
}
