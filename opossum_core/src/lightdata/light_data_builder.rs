//! Builder for [`LightData`]
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

use super::{LightData, energy_data_builder::EnergyDataBuilder, ray_data_builder::RayDataBuilder};
use crate::{
    energy_distributions::EnergyDistType,
    error::OpmResult,
    lightdata::ray_data_builder::{CollimatedSrc, ImageSrc, PointSrc},
    position_distributions::PosDistType,
    spectral_distribution::SpecDistType,
    utils::default_from_name::DefaultFromName,
};

/// Builder for the generation of [`LightData`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum LightDataBuilder {
    /// Builder for the generation of [`LightData::Energy`].
    Energy(EnergyDataBuilder),
    /// Builder for the generation of [`LightData::Geometric`].
    Geometric(RayDataBuilder),
    // /// Dummy Fourier
    // Fourier,
}

// impl Validate for LightDataBuilder{
//     fn validate(&self) -> OpmResult<()> {
//         match self{
//             LightDataBuilder::Energy(energy_data_builder) => energy_data_builder.validate(),
//             LightDataBuilder::Geometric(ray_data_builder) => ray_data_builder.validate(),
//         }
//     }
// }

impl DefaultFromName for LightDataBuilder {}

impl Default for LightDataBuilder {
    fn default() -> Self {
        Self::Geometric(RayDataBuilder::default())
    }
}

impl LightDataBuilder {
    /// Create [`LightData`] from the builder definition.
    ///
    /// # Errors
    ///
    /// This function will return an error if the concrete implementation of the builder fails.
    pub fn build(self) -> OpmResult<LightData> {
        match self {
            Self::Energy(e) => e.build(),
            Self::Geometric(r) => r.build(),
            // Self::Fourier => Ok(LightData::Fourier),
        }
    }
    /// Get the position distribution type, if applicable.
    ///
    /// Returns the [`PosDistType`] used in the ray-based (geometric) light source,
    /// if the source variant supports it. This is typically available for
    /// collimated and point sources only.
    ///
    /// Returns `None` if the builder is using an `Energy` source or a geometric
    /// variant that does not support positional distribution (e.g., `Raw` or `Image`).
    ///
    /// # Returns
    /// - `Some(PosDistType)` if available.
    /// - `None` otherwise.
    #[must_use]
    pub const fn get_position_distribution_type(&self) -> Option<PosDistType> {
        match self {
            Self::Energy(_) => None,
            Self::Geometric(ray_data_builder) => ray_data_builder.get_position_distribution_type(),
        }
    }
    /// Get the energy distribution type, if applicable.
    ///
    /// Returns the [`EnergyDistType`] used in the ray-based (geometric) light source,
    /// if the source variant supports it. Only collimated and point sources expose this data.
    ///
    /// Returns `None` if the builder is using an `Energy` source or an unsupported geometric variant.
    ///
    /// # Returns
    /// - `Some(EnergyDistType)` if available.
    /// - `None` otherwise.
    #[must_use]
    pub const fn get_energy_distribution_type(&self) -> Option<EnergyDistType> {
        match self {
            Self::Energy(_) => None,
            Self::Geometric(ray_data_builder) => ray_data_builder.get_energy_distribution_type(),
        }
    }

    /// Get the spectral distribution type, if applicable.
    ///
    /// Returns the [`SpecDistType`] used in the ray-based (geometric) light source,
    /// if the source variant supports it. Available for collimated and point sources.
    ///
    /// Returns `None` if the builder is using an `Energy` source or a geometric
    /// variant without spectral configuration (e.g., `Raw` or `Image`).
    ///
    /// # Returns
    /// - `Some(SpecDistType)` if available.
    /// - `None` otherwise.
    #[must_use]
    pub fn get_spectral_distribution_type(&self) -> Option<SpecDistType> {
        match self {
            Self::Energy(_) => None,
            Self::Geometric(ray_data_builder) => ray_data_builder.get_spectral_distribution_type(),
        }
    }
}

impl Display for LightDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Energy(_) => write!(f, "Energy"),
            Self::Geometric(_) => write!(f, "Rays"),
            // Self::Fourier => write!(f, "Fourier"),
        }
    }
}

impl From<ImageSrc> for LightDataBuilder {
    fn from(value: ImageSrc) -> Self {
        Self::Geometric(RayDataBuilder::Image(value))
    }
}

impl From<PointSrc> for LightDataBuilder {
    fn from(value: PointSrc) -> Self {
        Self::Geometric(RayDataBuilder::PointSrc(value))
    }
}

impl From<CollimatedSrc> for LightDataBuilder {
    fn from(value: CollimatedSrc) -> Self {
        Self::Geometric(RayDataBuilder::Collimated(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        joule, lightdata::energy_data_builder::EnergyLaserLines, nanometer, properties::Proptype,
        rays::Rays,
    };

    #[test]
    fn from_light_data_builder_to_proptype() {
        let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(1000.0), joule!(1.0))], nanometer!(1.0))
                .unwrap(),
        ));
        let proptype: Proptype = light_data_builder.into();
        assert!(matches!(proptype, Proptype::LightDataBuilder(_)));
    }
    #[test]
    fn display_light_data_builder() {
        let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(1000.0), joule!(1.0))], nanometer!(1.0))
                .unwrap(),
        ));
        assert_eq!(format!("{light_data_builder}"), "Energy");
    }
    #[test]
    fn build_light_data() {
        let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(1000.0), joule!(1.0))], nanometer!(1.0))
                .unwrap(),
        ));
        let light_data = light_data_builder.build().unwrap();
        assert!(matches!(light_data, LightData::Energy(_)));
        // let light_data_builder = LightDataBuilder::Fourier;
        // let light_data = light_data_builder.build().unwrap();
        // assert!(matches!(light_data, LightData::Fourier));
        let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Raw(Rays::default()));
        let light_data = light_data_builder.build().unwrap();
        assert!(matches!(light_data, LightData::Geometric(_)));
    }
}
