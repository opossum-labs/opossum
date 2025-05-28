use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Structure holding the version information
#[derive(ToSchema, Serialize, Deserialize)]
pub struct VersionInfo {
    /// version of the OPOSSUM API backend
    #[schema(example = "0.1.0")]
    backend_version: String,
    /// version of the OPOSSUM library (possibly including the git hash)
    #[schema(example = "0.6.0-18-g80cb67f (2025/02/19 15:29)")]
    opossum_version: String,
}