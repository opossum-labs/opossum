//! Module for handling optical materials in `opossum_core`.

use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use uuid::Uuid;

use crate::{
    asset::AssetHeader,
    error::OpmResult,
    refractive_index::{
        RefrIndexAir, RefrIndexConrady, RefrIndexConst, RefrIndexSchott, RefrIndexSellmeier1,
        RefractiveIndexType,
    },
};

/// Primary optical properties required for optical simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticalProperties {
    /// Refractive index calculation model.
    pub refractive_index: RefractiveIndexType,

    /// Optional constant absorption coefficient (e.g., in 1/m).
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

/// Optional thermal properties of a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalProperties {
    /// Thermal conductivity (e.g., in W/(m*K)).
    #[serde(default)]
    pub thermal_conductivity: Option<f64>,

    /// Coefficient of thermal expansion (e.g., in 1/K).
    #[serde(default)]
    pub expansion_coefficient: Option<f64>,
}

/// Optional mechanical properties of a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MechanicalProperties {
    /// Mass density (e.g., in kg/m^3).
    #[serde(default)]
    pub density: Option<f64>,

    /// Young's modulus (e.g., in Pa).
    #[serde(default)]
    pub youngs_modulus: Option<f64>,
}

/// Represents a complete optical material embedded in an OPOSSUM scenery or stored in the registry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    /// Shared metadata header (UUID, versioning, name, vendor).
    pub header: AssetHeader,

    /// Primary optical properties.
    pub optical: OpticalProperties,

    /// Optional thermal properties block.
    #[serde(default)]
    pub thermal: Option<ThermalProperties>,

    /// Optional mechanical properties block.
    #[serde(default)]
    pub mechanical: Option<MechanicalProperties>,
}

impl Material {
    /// Creates a new `Material` instance with explicit ID, version, and metadata.
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

    /// Creates an ad-hoc local material with a newly generated UUID, version 0, and default metadata.
    pub fn new_custom(name: impl Into<String>, refractive_index: RefractiveIndexType) -> Self {
        Self::new(Uuid::new_v4(), 0, name, None, None, refractive_index)
    }

    /// Returns the unique ID of the material.
    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.header.id
    }

    /// Returns the version number of the material.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.header.version
    }

    /// Returns the name of the material.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.header.name
    }

    /// Calculates the refractive index for a given wavelength.
    ///
    /// # Errors
    /// Returns an error if calculation fails or wavelength is out of bounds.
    pub fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        self.optical
            .refractive_index
            .get_refractive_index(wavelength)
    }
}

impl Default for Material {
    fn default() -> Self {
        Self::new_custom(
            "Default Glass",
            RefractiveIndexType::Const(RefrIndexConst::new(1.5).unwrap()),
        )
    }
}

// --- From implementations for owned and borrowed RefractiveIndexType ---

impl From<RefractiveIndexType> for Material {
    fn from(refr: RefractiveIndexType) -> Self {
        Self::new_custom("Custom Material", refr)
    }
}

impl From<&RefractiveIndexType> for Material {
    fn from(refr: &RefractiveIndexType) -> Self {
        Self::new_custom("Custom Material", refr.clone())
    }
}

// --- From implementations for RefrIndexConst (owned & borrowed) ---

impl From<RefrIndexConst> for Material {
    fn from(refr: RefrIndexConst) -> Self {
        Self::new_custom("Custom Material", RefractiveIndexType::Const(refr))
    }
}

impl From<&RefrIndexConst> for Material {
    fn from(refr: &RefrIndexConst) -> Self {
        Self::new_custom("Custom Material", RefractiveIndexType::Const(*refr))
    }
}

// --- From implementations for other concrete dispersion models ---

impl From<RefrIndexSellmeier1> for Material {
    fn from(refr: RefrIndexSellmeier1) -> Self {
        Self::new_custom("Custom Material", RefractiveIndexType::Sellmeier1(refr))
    }
}

impl From<&RefrIndexSellmeier1> for Material {
    fn from(refr: &RefrIndexSellmeier1) -> Self {
        Self::new_custom(
            "Custom Material",
            RefractiveIndexType::Sellmeier1(refr.clone()),
        )
    }
}

impl From<RefrIndexSchott> for Material {
    fn from(refr: RefrIndexSchott) -> Self {
        Self::new_custom("Custom Material", RefractiveIndexType::Schott(refr))
    }
}

impl From<&RefrIndexSchott> for Material {
    fn from(refr: &RefrIndexSchott) -> Self {
        Self::new_custom("Custom Material", RefractiveIndexType::Schott(refr.clone()))
    }
}

impl From<RefrIndexConrady> for Material {
    fn from(refr: RefrIndexConrady) -> Self {
        Self::new_custom("Custom Material", RefractiveIndexType::Conrady(refr))
    }
}

impl From<&RefrIndexConrady> for Material {
    fn from(refr: &RefrIndexConrady) -> Self {
        Self::new_custom(
            "Custom Material",
            RefractiveIndexType::Conrady(refr.clone()),
        )
    }
}

impl From<RefrIndexAir> for Material {
    fn from(refr: RefrIndexAir) -> Self {
        Self::new_custom("Custom Material", RefractiveIndexType::Air(refr))
    }
}

impl From<&RefrIndexAir> for Material {
    fn from(refr: &RefrIndexAir) -> Self {
        Self::new_custom("Custom Material", RefractiveIndexType::Air(refr.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::nanometer;

    #[test]
    fn test_custom_material_creation() -> OpmResult<()> {
        let const_refr = RefrIndexConst::new(1.5)?;
        let material = Material::new_custom("Test Glass", const_refr.into());

        assert_eq!(material.name(), "Test Glass");
        assert_eq!(material.version(), 0);

        let wvl = Length::new::<nanometer>(550.0);
        let n = material.get_refractive_index(wvl)?;
        assert_eq!(n, 1.5);

        Ok(())
    }
}
