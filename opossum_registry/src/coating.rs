use opossum_core::{asset::AssetHeader, coatings::CoatingType};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::asset::RegisterableAsset;

/// Represents an optical coating asset managed by `opossum_registry`.
///
/// Wraps the shared [`AssetHeader`] alongside the concrete coating model
/// from `opossum_core::coating::CoatingType`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoatingAsset {
    /// Shared metadata header (UUID, schema version, asset version, name, manufacturer).
    pub header: AssetHeader,

    /// Concrete optical coating model from `opossum_core`.
    pub coating: CoatingType,
}

impl CoatingAsset {
    /// Creates a new `CoatingAsset` using the current schema version.
    pub fn new(
        id: Uuid,
        version: u32,
        name: impl Into<String>,
        manufacturer: Option<String>,
        description: Option<String>,
        coating: CoatingType,
    ) -> Self {
        Self {
            header: AssetHeader::new(id, version, name, manufacturer, description),
            coating,
        }
    }
}

impl RegisterableAsset for CoatingAsset {
    fn header(&self) -> &AssetHeader {
        &self.header
    }

    fn relative_subfolder() -> &'static str {
        "coatings"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::asset::CURRENT_SCHEMA_VERSION;
    use opossum_core::coatings::CoatingConstantR;
    use uom::si::f64::Ratio;
    use uom::si::ratio::percent;

    #[test]
    fn test_ideal_ar_coating_creation() {
        let id = Uuid::new_v4();
        let coating_asset = CoatingAsset::new(
            id,
            1,
            "Ideal AR Coating",
            Some("Thorlabs".to_string()),
            Some("Perfect anti-reflective coating".to_string()),
            CoatingType::IdealAR,
        );

        assert_eq!(coating_asset.id(), id);
        assert_eq!(coating_asset.version(), 1);
        assert_eq!(coating_asset.name(), "Ideal AR Coating");
        assert_eq!(coating_asset.manufacturer(), Some("Thorlabs"));
        assert_eq!(coating_asset.coating, CoatingType::IdealAR);
        assert_eq!(CoatingAsset::relative_subfolder(), "coatings");
    }

    #[test]
    fn test_constant_r_coating_creation() {
        let id = Uuid::new_v4();
        let reflectivity = Ratio::new::<percent>(50.0);
        let constant_r = CoatingConstantR::new(reflectivity)
            .expect("Failed to create constant reflectivity model");

        let coating_asset = CoatingAsset::new(
            id,
            1,
            "50/50 Beamsplitter Coating",
            Some("Edmund Optics".to_string()),
            None,
            CoatingType::ConstantR(constant_r),
        );

        if let CoatingType::ConstantR(config) = &coating_asset.coating {
            assert_eq!(config.reflectivity(), reflectivity);
        } else {
            panic!("Expected CoatingType::ConstantR variant");
        }
    }

    #[test]
    fn test_fresnel_coating_creation() {
        let id = Uuid::new_v4();
        let coating_asset = CoatingAsset::new(
            id,
            1,
            "Uncoated Surface (Fresnel)",
            None,
            None,
            CoatingType::Fresnel,
        );

        assert_eq!(coating_asset.coating, CoatingType::Fresnel);
    }

    #[test]
    fn test_coating_ron_serialization_roundtrip() {
        let id = Uuid::nil();
        let reflectivity = Ratio::new::<percent>(99.5);
        let constant_r = CoatingConstantR::new(reflectivity).expect("Valid reflectivity");

        let coating_asset = CoatingAsset::new(
            id,
            1,
            "HR Mirror Coating",
            Some("CVI".to_string()),
            None,
            CoatingType::ConstantR(constant_r),
        );

        // Serialize to RON
        let ron_str = ron::ser::to_string_pretty(&coating_asset, ron::ser::PrettyConfig::default())
            .expect("Failed to serialize CoatingAsset to RON");

        assert!(ron_str.contains("HR Mirror Coating"));
        assert!(ron_str.contains("ConstantR"));

        // Deserialize back from RON
        let deserialized: CoatingAsset =
            ron::from_str(&ron_str).expect("Failed to deserialize CoatingAsset from RON");

        assert_eq!(coating_asset, deserialized);
    }

    #[test]
    fn test_coating_forward_compatibility() {
        // Simulated RON representation from a future version (Schema V2) with unknown field 'durability_class'
        let future_ron = r#"
        (
            header: (
                schema_version: 2,
                id: "00000000-0000-0000-0000-000000000000",
                version: 1,
                name: "Future Coating",
                manufacturer: None,
                description: None,
            ),
            coating: IdealAR,
            durability_class: "MIL-C-675C",
        )
        "#;

        let asset: CoatingAsset =
            ron::from_str(future_ron).expect("Forward compatibility parsing failed");

        assert_eq!(asset.name(), "Future Coating");
        assert_eq!(asset.coating, CoatingType::IdealAR);
        assert!(asset.schema_version() > CURRENT_SCHEMA_VERSION);
    }
}
