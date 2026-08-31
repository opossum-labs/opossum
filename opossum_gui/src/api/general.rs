//! General api calls
use crate::HTTP_API_CLIENT;
use opossum_core::prelude::*;
use opossum_core::types::api_types::{NodeType, VersionInfo};

/// Send a request to check if the backend url is reachable and corresponds to the opossum backend.
///
/// # Errors
///
/// This function will return an error if
/// - the request fails (e.g. the base url is not reachable)
/// - the response cannot be deserialized into a string
#[allow(dead_code)]
pub async fn get_api_welcome() -> Result<String, String> {
    HTTP_API_CLIENT().get_raw("/api/").await
}

/// Send reqeust to get the version of the opossum backend and the opossum library.
///
/// # Errors
///
/// This function will return an error if
/// - the `VersionInfo` struct cannot be deserialized
pub async fn get_version() -> Result<VersionInfo, String> {
    HTTP_API_CLIENT().get::<VersionInfo>("/api/version").await
}

/// Send a request to get all available node types.
///
/// # Errors
///
/// This function will return an error if
/// - the response cannot be deserialized into a vector of [`NodeType`] structs.
pub async fn get_node_types() -> Result<Vec<NodeType>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<NodeType>>("/api/node_types")
        .await
}

/// Send a request to get all available anaylzer types.
///
/// # Errors
///
/// This function will return an error if
/// - the response cannot be deserialized into a vector of [`AnalyzerType`] structs.
pub async fn get_analyzer_types() -> Result<Vec<AnalyzerType>, String> {
    HTTP_API_CLIENT()
        .get::<Vec<AnalyzerType>>("/api/analyzer_types")
        .await
}

// /// Send a request to analyze current setup.
// ///
// /// # Errors
// ///
// /// This function will return an error if
// /// - the response cannot be deserialized into a vector of [`AnalyzerType`] structs.
// // pub async fn analyze(client: &HTTPClient) -> Result<Vec<AnalysisReport>, String> {
// //     client.get::<Vec<AnalysisReport>>("/api/analyze").await
// // }

/// Send a request to shutdown the backend server.
///
/// This function shuts down the backend server. No further communication is possible after this call.
#[allow(dead_code)]
pub async fn post_terminate() {
    let _ = HTTP_API_CLIENT()
        .post::<String, String>("/api/terminate", String::new())
        .await;
}
