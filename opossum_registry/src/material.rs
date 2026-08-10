use opossum_core::refractive_index::RefractiveIndexType;
use serde::{Deserialize, Serialize};
use uom::si::f64::{MassDensity, Pressure, TemperatureCoefficient, ThermalConductivity};
use uuid::Uuid;

use crate::asset::{AssetHeader, RegisterableAsset};

/// Optical properties of a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticalProperties {
    /// Refractive index model from `opossum_core`.
    pub refractive_index: RefractiveIndexType,

    /// Optional absorption coefficient.
    #[serde(default)]
    pub absorption: Option<f64>,
}

impl OpticalProperties {
    /// Creates a new `OpticalProperties` container.
    #[must_use]
    pub const fn new(refractive_index: RefractiveIndexType) -> Self {
        Self {
            refractive_index,
            absorption: None,
        }
    }
}

/// Thermal properties of a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalProperties {
    /// Thermal conductivity (e.g., in W/(m*K)).
    #[serde(default)]
    pub thermal_conductivity: Option<ThermalConductivity>,

    /// Coefficient of thermal expansion (e.g., in 1/K).
    #[serde(default)]
    pub expansion_coefficient: Option<TemperatureCoefficient>,
}

/// Mechanical properties of a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MechanicalProperties {
    /// Mass density
    #[serde(default)]
    pub density: Option<MassDensity>,

    /// Young's modulus
    #[serde(default)]
    pub youngs_modulus: Option<Pressure>,
}

/// Represents a complete optical material asset stored in the registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialAsset {
    /// Common metadata header (UUID, versions, name, manufacturer).
    pub header: AssetHeader,

    /// Primary optical properties (required for optical simulation).
    pub optical: OpticalProperties,

    /// Optional thermal properties block.
    #[serde(default)]
    pub thermal: Option<ThermalProperties>,

    /// Optional mechanical properties block.
    #[serde(default)]
    pub mechanical: Option<MechanicalProperties>,
}

impl MaterialAsset {
    /// Creates a new `MaterialAsset` with default optical properties.
    pub fn new(
        id: Uuid,
        version: u32,
        name: impl Into<String>,
        manufacturer: Option<String>,
        description: Option<String>,
        refractive_index: RefractiveIndexType,
    ) -> Self {
        Self {
            header: AssetHeader::new(id, version, name, manufacturer, description),
            optical: OpticalProperties::new(refractive_index),
            thermal: None,
            mechanical: None,
        }
    }
}

impl RegisterableAsset for MaterialAsset {
    fn header(&self) -> &AssetHeader {
        &self.header
    }

    fn relative_subfolder() -> &'static str {
        "materials"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::refractive_index::RefrIndexConst;
    use uom::si::f64::Length;
    use uom::si::length::nanometer;
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;

    #[test]
    fn test_structured_material_creation() {
        let id = Uuid::new_v4();
        let const_index = RefractiveIndexType::Const(
            RefrIndexConst::new(1.5).expect("Failed to create constant index"),
        );

        let mut material = MaterialAsset::new(
            id,
            1,
            "N-BK7",
            Some("Schott".to_string()),
            None,
            const_index,
        );

        // Add thermal properties
        material.thermal = Some(ThermalProperties {
            thermal_conductivity: Some(ThermalConductivity::new::<watt_per_meter_kelvin>(1.114)),
            expansion_coefficient: Some(TemperatureCoefficient::new::<per_kelvin>(7.1e-6)),
        });

        assert_eq!(material.name(), "N-BK7");
        assert_eq!(
            material
                .thermal
                .as_ref()
                .and_then(|t| t.thermal_conductivity),
            Some(ThermalConductivity::new::<watt_per_meter_kelvin>(1.114))
        );
        assert_eq!(material.mechanical, None);
    }
    #[test]
    fn test_material_refractive_index_calculation() {
        let id = Uuid::new_v4();
        let const_index = RefractiveIndexType::Const(
            RefrIndexConst::new(1.5).expect("Failed to create constant index"),
        );

        let material = MaterialAsset::new(id, 1, "Constant Glass", None, None, const_index);

        let wvl = Length::new::<nanometer>(550.0);
        let n = material
            .optical
            .refractive_index
            .get_refractive_index(wvl)
            .expect("Calculation failed");

        assert_eq!(n, 1.5);
    }

    #[test]
    fn test_structured_ron_roundtrip() {
        let id = Uuid::nil();
        let const_index = RefractiveIndexType::Const(
            RefrIndexConst::new(1.458).expect("Failed to create constant index"),
        );

        let mut material = MaterialAsset::new(id, 1, "Fused Silica", None, None, const_index);
        material.optical.absorption = Some(0.001);

        let ron_str = ron::ser::to_string_pretty(&material, ron::ser::PrettyConfig::default())
            .expect("Serialization failed");

        let deserialized: MaterialAsset = ron::from_str(&ron_str).expect("Deserialization failed");

        assert_eq!(material, deserialized);
    }

    #[test]
    fn test_backward_compatibility_missing_optional_fields() {
        // RON string representing an older material definition without the `absorption` field
        let legacy_ron = r#"
        (
            header: (
                schema_version: 1,
                id: "00000000-0000-0000-0000-000000000000",
                version: 1,
                name: "Legacy Glass",
                manufacturer: None,
                description: None,
            ),
            optical: (
                refractive_index: Const((
                    refractive_index: 1.52,
                )),
            ),
        )
        "#;

        let material: MaterialAsset =
            ron::from_str(legacy_ron).expect("Failed to parse legacy RON string");

        assert_eq!(material.name(), "Legacy Glass");
        assert_eq!(material.optical.absorption, None);
    }
    #[test]
    fn test_forward_compatibility_newer_schema_and_unknown_fields() {
        use crate::asset::CURRENT_SCHEMA_VERSION;

        // Simulated RON string generated by a future version of OPOSSUM (Schema Version 2).
        // It includes a higher schema_version and a new top-level field `thermal_expansion`.
        let future_ron = r#"
        (
            header: (
                schema_version: 2,
                id: "11111111-1111-1111-1111-111111111111",
                version: 1,
                name: "Future Crown Glass",
                manufacturer: Some("Schott"),
                description: Some("Created with a future version of OPOSSUM"),
            ),
            optical: (
                refractive_index: Const((
                    refractive_index: 1.52,
                )),
            ),
            // New field introduced in Schema V2 that Schema V1 does not know about
            blahblah: 0.0000071,
        )
        "#;

        // Step 1: Attempt deserialization into our current V1 MaterialAsset struct.
        // Serde ignores unknown fields by default, so this operation must succeed.
        let result: Result<MaterialAsset, _> = ron::from_str(future_ron);
        assert!(
            result.is_ok(),
            "Deserialization failed for a document from a newer schema version!"
        );

        let material = result.unwrap();

        // Step 2: Verify that standard V1 fields were parsed correctly.
        assert_eq!(material.name(), "Future Crown Glass");
        assert_eq!(material.manufacturer(), Some("Schott"));
        assert_eq!(material.version(), 1);

        // Step 3: Check that the schema version is correctly identified as newer.
        // This allows the application layer (GUI/CLI) to trigger a non-blocking warning.
        assert_eq!(material.schema_version(), 2);
        assert!(
            material.schema_version() > CURRENT_SCHEMA_VERSION,
            "The document's schema version should be higher than CURRENT_SCHEMA_VERSION"
        );
    }
}
