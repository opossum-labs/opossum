//! Material management and definitions
//!
//! This module defines the composite [`Material`] struct as well as individual property
//! subsets: optical, thermal, and mechanical properties.

pub mod mechanical;
pub mod optical;
pub mod thermal;

pub use mechanical::MechanicalProperties;
pub use optical::OpticalProperties;
pub use thermal::ThermalProperties;

use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use uuid::Uuid;

use crate::{
    asset::AssetHeader,
    error::OpmResult,
    refractive_index::{
        RefrIndexAir, RefrIndexConrady, RefrIndexConst, RefrIndexSchott, RefrIndexSellmeier1,
        RefractiveIndexType, refr_index_vaccuum,
    },
};

/// Represents a complete material embedded in an OPOSSUM scenery or stored in the registry.
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
    /// Creates a new material draft with a random UUID and version 0.
    ///
    /// Version 0 indicates that this material is a local draft and has not yet
    /// been published to the registry.
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
    ///
    /// Keeps the identical UUID to maintain identity, but resets the version to 0
    /// so the registry loader knows it must assign the next available version number upon publishing.
    #[must_use]
    pub fn new_draft_from(&self) -> Self {
        let mut draft = self.clone();
        draft.header.version = 0;
        draft
    }

    /// Creates a [`Material`] representing vacuum with standard refractive index 1.0.
    #[must_use]
    pub fn vacuum() -> Self {
        let mut material = Self::new_draft("vacuum", None, None, refr_index_vaccuum());
        material.header.id = Uuid::nil();
        material
    }

    /// Creates a [`Material`] representing standard air.
    #[must_use]
    pub fn material_air() -> Self {
        let mut material: Self = RefractiveIndexType::Air(RefrIndexAir::default()).into();
        material.header.name = "air".to_string();
        material
    }

    /// Creates an independent ad-hoc copy with a new random UUID and version 0.
    ///
    /// This detaches the material from any catalog identity.
    #[must_use]
    pub fn clone_as_adhoc(&self) -> Self {
        let mut adhoc = self.clone();
        adhoc.header.id = Uuid::new_v4();
        adhoc.header.version = 0;
        adhoc
    }

    /// Calculates the refractive index for a given wavelength.
    ///
    /// # Errors
    ///
    /// Returns an [`OpmResult::Err`] if the dispersion model calculation fails or the
    /// wavelength is outside the valid model boundaries.
    pub fn refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        self.optical
            .refractive_index
            .get_refractive_index(wavelength)
    }

    /// Calculates the internal optical transmission for a given wavelength and path length.
    ///
    /// # Errors
    ///
    /// Returns an [`OpmResult::Err`] if the absorption model fails to evaluate transmission.
    pub fn transmission(&self, wavelength: Length, path_length: Length) -> OpmResult<f64> {
        self.optical
            .absorption
            .transmittance(wavelength, path_length)
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

    /// Calculates the refractive index for a given wavelength.
    ///
    /// # Errors
    ///
    /// Returns an [`OpmResult::Err`] if the dispersion formula fails or the wavelength
    /// is outside the validity range.
    pub fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        self.refractive_index(wavelength)
    }

    /// Returns a reference to the refractive index type of this [`Material`].
    #[must_use]
    pub const fn refractive_index_type(&self) -> &RefractiveIndexType {
        &self.optical.refractive_index
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

// --- From implementations for RefractiveIndexType ---

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

// --- From implementations for RefrIndexConst ---

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

// --- From implementations for dispersion models ---

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
    use uom::si::length::{millimeter, nanometer};

    #[test]
    fn test_custom_material_creation() -> OpmResult<()> {
        let const_refr = RefrIndexConst::new(1.5)?;
        let material = Material::new_draft("Test Glass", None, None, const_refr.into());

        assert_eq!(material.name(), "Test Glass");
        assert_eq!(material.version(), 0);

        let wvl = Length::new::<nanometer>(550.0);
        let n = material.get_refractive_index(wvl)?;
        assert!((n - 1.5).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_vacuum_and_air_constructors() -> OpmResult<()> {
        let vacuum = Material::vacuum();
        assert_eq!(vacuum.id(), Uuid::nil());
        assert_eq!(vacuum.name(), "vacuum");

        let air = Material::material_air();
        assert_eq!(air.name(), "air");

        let wvl = Length::new::<nanometer>(589.3);
        assert!((vacuum.refractive_index(wvl)? - 1.0).abs() < 1e-12);

        Ok(())
    }

    #[test]
    fn test_draft_and_adhoc_cloning() {
        let const_refr = RefrIndexConst::new(1.5).unwrap();
        let original =
            Material::new_for_test(Uuid::new_v4(), 3, "Catalog Material", const_refr.into());

        let draft = original.new_draft_from();
        assert_eq!(draft.id(), original.id());
        assert_eq!(draft.version(), 0);

        let adhoc = original.clone_as_adhoc();
        assert_ne!(adhoc.id(), original.id());
        assert_eq!(adhoc.version(), 0);
    }

    #[test]
    fn test_default_transmission() -> OpmResult<()> {
        let material = Material::default();
        let wvl = Length::new::<nanometer>(550.0);
        let path = Length::new::<millimeter>(10.0);

        let t = material.transmission(wvl, path)?;
        assert_eq!(t, 1.0);

        Ok(())
    }
}
