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

/// Name of the property that carries the [`Material`] of a node with a volume.
///
/// Same purpose as [`CLEAR_APERTURE`](crate::geometry::body::CLEAR_APERTURE) for the transversal
/// extent: the node declarations and every reader refer to the property by this constant rather
/// than by a literal.
pub const MATERIAL: &str = "Material";

/// Name the [`MATERIAL`] property had before it carried a whole [`Material`].
///
/// Up to and including OPOSSUM 0.7.2 the same slot held a bare
/// [`RefractiveIndexType`] under this name. The constant is kept so that `.opm` files written by
/// an older OPOSSUM can be migrated on load (see `migrate_legacy_properties` in
/// [`properties`](crate::properties)) — without that,
/// [`Properties::update`](crate::properties::Properties::update) would silently drop the old key
/// and the node would fall back to its default material.
pub const LEGACY_REFRACTIVE_INDEX: &str = "refractive index";

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
    #[serde(flatten)]
    pub header: AssetHeader,

    /// Primary optical properties.
    pub optical: OpticalProperties,

    /// Optional thermal properties block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal: Option<ThermalProperties>,

    /// Optional mechanical properties block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mechanical: Option<MechanicalProperties>,
}

impl Material {
    /// Create a new [`Material`]
    ///
    /// Creates a completely new material draft with a random UUID and version 0.
    /// Version 0 indicates that this material is a local draft and has not yet been published to the registry.
    #[must_use]
    pub fn new_draft(
        name: impl Into<String>,
        manufacturer: Option<String>,
        description: Option<String>,
        refractive_index: RefractiveIndexType,
    ) -> Self {
        Self {
            header: AssetHeader::new(Uuid::new_v4(), 0, name, manufacturer, description),
            optical: OpticalProperties::new(refractive_index),
            thermal: None,
            mechanical: None,
        }
    }

    /// Creates a new draft based on an existing material (for updates).
    /// Keeps the identical UUID to maintain identity, but resets the version to 0
    /// so the registry loader knows it must assign the next available version number upon publishing.
    #[must_use]
    pub fn new_draft_from(&self) -> Self {
        let mut draft = self.clone();
        draft.header.version = 0; // Mark as unsaved draft
        draft
    }

    /// Creates an independent ad-hoc copy with a new random UUID and version 0.
    /// This detaches the material from any catalog identity.
    #[must_use]
    pub fn clone_as_adhoc(&self) -> Self {
        let mut adhoc = self.clone();
        adhoc.header.id = Uuid::new_v4();
        adhoc.header.version = 0;
        adhoc
    }

    /// For testing purposes only: Creates a material with a specific UUID and version.
    #[cfg(test)]
    pub fn new_for_test(
        id: Uuid,
        version: u32,
        name: impl Into<String>,
        refractive_index: RefractiveIndexType,
    ) -> Self {
        Self {
            header: AssetHeader::new(id, version, name, None, None),
            optical: OpticalProperties::new(refractive_index),
            thermal: None,
            mechanical: None,
        }
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

    /// Returns the refractive index model n(λ) of the material.
    ///
    /// This hands out the model itself rather than a value at one wavelength, for the callers that
    /// have to pass the whole dispersion model on (e.g. the volume propagation during ray tracing).
    #[must_use]
    pub const fn refractive_index(&self) -> &RefractiveIndexType {
        &self.optical.refractive_index
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
        Self::new_draft(
            "Default Glass",
            None,
            None,
            RefrIndexConst::new(1.5).unwrap().into(),
        )
    }
}

// --- From implementations for owned and borrowed RefractiveIndexType ---

impl From<RefractiveIndexType> for Material {
    fn from(refr: RefractiveIndexType) -> Self {
        Self::new_draft("Custom Material", None, None, refr)
    }
}

impl From<&RefractiveIndexType> for Material {
    fn from(refr: &RefractiveIndexType) -> Self {
        Self::new_draft("Custom Material", None, None, refr.clone())
    }
}

// --- From implementations for RefrIndexConst (owned & borrowed) ---

impl From<RefrIndexConst> for Material {
    fn from(refr: RefrIndexConst) -> Self {
        Self::new_draft("Custom Material", None, None, refr.into())
    }
}

impl From<&RefrIndexConst> for Material {
    fn from(refr: &RefrIndexConst) -> Self {
        Self::new_draft("Custom Material", None, None, (*refr).into())
    }
}

// --- From implementations for other concrete dispersion models ---

impl From<RefrIndexSellmeier1> for Material {
    fn from(refr: RefrIndexSellmeier1) -> Self {
        Self::new_draft("Custom Material", None, None, refr.into())
    }
}

impl From<&RefrIndexSellmeier1> for Material {
    fn from(refr: &RefrIndexSellmeier1) -> Self {
        Self::new_draft("Custom Material", None, None, refr.clone().into())
    }
}

impl From<RefrIndexSchott> for Material {
    fn from(refr: RefrIndexSchott) -> Self {
        Self::new_draft("Custom Material", None, None, refr.into())
    }
}

impl From<&RefrIndexSchott> for Material {
    fn from(refr: &RefrIndexSchott) -> Self {
        Self::new_draft("Custom Material", None, None, refr.clone().into())
    }
}

impl From<RefrIndexConrady> for Material {
    fn from(refr: RefrIndexConrady) -> Self {
        Self::new_draft("Custom Material", None, None, refr.into())
    }
}

impl From<&RefrIndexConrady> for Material {
    fn from(refr: &RefrIndexConrady) -> Self {
        Self::new_draft("Custom Material", None, None, refr.clone().into())
    }
}

impl From<RefrIndexAir> for Material {
    fn from(refr: RefrIndexAir) -> Self {
        Self::new_draft("Custom Material", None, None, refr.into())
    }
}

impl From<&RefrIndexAir> for Material {
    fn from(refr: &RefrIndexAir) -> Self {
        Self::new_draft("Custom Material", None, None, refr.clone().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::nanometer;

    #[test]
    fn test_custom_material_creation() -> OpmResult<()> {
        let const_refr = RefrIndexConst::new(1.5)?;
        let material = Material::new_draft("Test Glass", None, None, const_refr.into());

        assert_eq!(material.name(), "Test Glass");
        assert_eq!(material.version(), 0);

        let wvl = Length::new::<nanometer>(550.0);
        let n = material.get_refractive_index(wvl)?;
        assert_eq!(n, 1.5);

        Ok(())
    }
}
