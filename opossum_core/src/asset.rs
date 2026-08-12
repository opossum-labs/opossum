use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current schema version supported by this build of OPOSSUM.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Common metadata header shared by all registry assets (Materials, Coatings, Components, Light Sources, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetHeader {
    /// Format schema version for compatibility checks.
    pub schema_version: u32,

    /// Unique global identifier for the asset instance.
    pub id: Uuid,

    /// Data version of this asset (follows the append-only versioning strategy).
    pub version: u32,

    /// Human-readable display name (e.g., "N-BK7", "LA1951-A").
    pub name: String,

    /// Optional manufacturer or vendor name (e.g., "Schott", "Thorlabs").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,

    /// Optional description or notes about the asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl AssetHeader {
    /// Creates a new `AssetHeader` instance with the current schema version.
    pub fn new(
        id: Uuid,
        version: u32,
        name: impl Into<String>,
        manufacturer: Option<String>,
        description: Option<String>,
    ) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            id,
            version,
            name: name.into(),
            manufacturer,
            description,
        }
    }
}
