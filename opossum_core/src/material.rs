//! Module for handling optical materials in `opossum_core`.

use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use uuid::Uuid;

use crate::{
    error::OpmResult,
    refractive_index::{
        RefrIndexAir, RefrIndexConrady, RefrIndexConst, RefrIndexSchott, RefrIndexSellmeier1,
        RefractiveIndexType,
    },
};

/// Represents an optical material embedded within an OPOSSUM scenery or document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    /// Unique identifier for this material instance.
    pub id: Uuid,

    /// Version of the material data (0 for ad-hoc/local materials, >=1 for registry assets).
    pub version: u32,

    /// Display name of the material (e.g., "N-BK7", "Custom Glass").
    pub name: String,

    /// Refractive index calculation model.
    pub refractive_index: RefractiveIndexType,

    /// Optional constant absorption coefficient (e.g., in 1/m).
    #[serde(default)]
    pub absorption: Option<f64>,
}

impl Material {
    /// Creates a new `Material` instance with explicit ID and version.
    pub fn new(
        id: Uuid,
        version: u32,
        name: impl Into<String>,
        refractive_index: RefractiveIndexType,
    ) -> Self {
        Self {
            id,
            version,
            name: name.into(),
            refractive_index,
            absorption: None,
        }
    }

    /// Creates an ad-hoc local material with a newly generated UUID and version 0.
    pub fn new_custom(name: impl Into<String>, refractive_index: RefractiveIndexType) -> Self {
        Self {
            id: Uuid::new_v4(),
            version: 0,
            name: name.into(),
            refractive_index,
            absorption: None,
        }
    }

    /// Calculates the refractive index for a given wavelength.
    ///
    /// # Errors
    /// Returns an error if calculation fails or wavelength is out of bounds.
    pub fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        self.refractive_index.get_refractive_index(wavelength)
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

        assert_eq!(material.name, "Test Glass");
        assert_eq!(material.version, 0);

        let wvl = Length::new::<nanometer>(550.0);
        let n = material.get_refractive_index(wvl)?;
        assert_eq!(n, 1.5);

        Ok(())
    }
}
