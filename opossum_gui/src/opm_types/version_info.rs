use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VersionInfo {
    /// version of the OPOSSUM API backend
    backend_version: String,
    /// version of the OPOSSUM library (possibly including the git hash)
    opossum_version: String,
}

impl VersionInfo {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn backend_version(&self) -> &str {
        &self.backend_version
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn opossum_version(&self) -> &str {
        &self.opossum_version
    }
}
