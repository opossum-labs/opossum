use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current schema version supported by this build of `opossum_registry`.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Common metadata header shared by all registry assets (Materials, Components, Light Sources, etc.).
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
    pub manufacturer: Option<String>,

    /// Optional description or notes about the asset.
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

/// Trait to be implemented by any type that can be stored and managed within `opossum_registry`.
pub trait RegisterableAsset: Serialize + for<'de> Deserialize<'de> {
    /// Returns a reference to the asset's shared header.
    fn header(&self) -> &AssetHeader;

    /// Returns the relative subfolder name in the registry repository (e.g., "materials").
    fn relative_subfolder() -> &'static str;

    /// Returns the schema version of the asset document.
    fn schema_version(&self) -> u32 {
        self.header().schema_version
    }

    /// Returns the unique ID of the asset.
    fn id(&self) -> Uuid {
        self.header().id
    }

    /// Returns the data version of the asset.
    fn version(&self) -> u32 {
        self.header().version
    }

    /// Returns the name of the asset.
    fn name(&self) -> &str {
        &self.header().name
    }

    /// Returns the optional manufacturer of the asset.
    fn manufacturer(&self) -> Option<&str> {
        self.header().manufacturer.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock asset struct used exclusively for unit testing the `RegisterableAsset` trait.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct MockAsset {
        pub header: AssetHeader,
        pub custom_property: f64,
    }

    impl RegisterableAsset for MockAsset {
        fn header(&self) -> &AssetHeader {
            &self.header
        }

        fn relative_subfolder() -> &'static str {
            "mock_assets"
        }
    }

    #[test]
    fn test_asset_header_creation_with_schema() {
        let id = Uuid::new_v4();
        let header = AssetHeader::new(id, 1, "Test Item", None, None);

        assert_eq!(header.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(header.id, id);
        assert_eq!(header.version, 1);
    }

    #[test]
    fn test_trait_schema_version_getter() {
        let id = Uuid::new_v4();
        let header = AssetHeader::new(id, 1, "Mock", None, None);
        let asset = MockAsset {
            header,
            custom_property: 1.0,
        };

        assert_eq!(asset.schema_version(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_registerable_asset_trait_methods() {
        let id = Uuid::new_v4();
        let header = AssetHeader::new(id, 2, "Lens A", Some("Thorlabs".to_string()), None);
        let mock_asset = MockAsset {
            header,
            custom_property: 42.0,
        };

        // Verify trait helper methods delegate correctly to the header
        assert_eq!(mock_asset.id(), id);
        assert_eq!(mock_asset.version(), 2);
        assert_eq!(mock_asset.name(), "Lens A");
        assert_eq!(mock_asset.manufacturer(), Some("Thorlabs"));
        assert_eq!(MockAsset::relative_subfolder(), "mock_assets");
    }

    #[test]
    fn test_asset_header_ron_serialization() {
        let id = Uuid::nil(); // Deterministic UUID for testing
        let header = AssetHeader::new(id, 1, "Static Asset", None, None);

        // Test serialization to RON format
        let ron_str = ron::to_string(&header).expect("Serialization failed");
        assert!(ron_str.contains("Static Asset"));

        // Test deserialization from RON format
        let deserialized: AssetHeader = ron::from_str(&ron_str).expect("Deserialization failed");
        assert_eq!(header, deserialized);
    }
}
