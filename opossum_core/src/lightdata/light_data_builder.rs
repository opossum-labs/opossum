//! Builder for [`LightData`]
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

use super::{LightData, energy_data_builder::EnergyDataBuilder, ray_data_source::RayDataSource};
use crate::{
    distributions::{energy::EnergyDistType, position::PosDistType, spectral::SpecDistType},
    error::OpmResult,
    lightdata::ray_data_source::{CollimatedSrc, ImageSrc, PointSrc},
    utils::default_from_name::DefaultFromName,
};

/// Builder for the generation of [`LightData`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter, EnsureValidated)]
pub enum LightDataBuilder {
    /// Builder for the generation of [`LightData::Energy`].
    Energy(EnergyDataBuilder),
    /// Builder for the generation of [`LightData::Geometric`].
    Geometric(RayDataSource),
    // /// Dummy Fourier
    // Fourier,
}

impl DefaultFromName for LightDataBuilder {}

impl Default for LightDataBuilder {
    fn default() -> Self {
        Self::Geometric(RayDataSource::default())
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
            Self::Energy(e) => Ok(LightData::Energy(e.build()?)),
            Self::Geometric(r) => Ok(LightData::Geometric(r.build()?)),
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

impl From<EnergyDataBuilder> for LightDataBuilder {
    fn from(value: EnergyDataBuilder) -> Self {
        Self::Energy(value)
    }
}
impl From<ImageSrc> for LightDataBuilder {
    fn from(value: ImageSrc) -> Self {
        Self::Geometric(RayDataSource::Image(value))
    }
}

impl From<PointSrc> for LightDataBuilder {
    fn from(value: PointSrc) -> Self {
        Self::Geometric(RayDataSource::PointSrc(value))
    }
}

impl From<CollimatedSrc> for LightDataBuilder {
    fn from(value: CollimatedSrc) -> Self {
        Self::Geometric(RayDataSource::Collimated(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
        joule,
        lightdata::energy_data_builder::EnergyLaserLines,
        nanometer,
        properties::Proptype,
        rays::Rays,
    };
    #[test]
    fn default() {
        let ldb = LightDataBuilder::default();
        assert_eq!(
            ldb.get_energy_distribution_type(),
            Some(UniformDist::default().into())
        );
        assert_eq!(
            ldb.get_position_distribution_type(),
            Some(Hexapolar::default().into())
        );
        assert_eq!(
            ldb.get_spectral_distribution_type(),
            Some(LaserLines::default().into())
        );
    }
    #[test]
    fn get_energy_distribution_type() {
        let edb = EnergyDataBuilder::default();
        let ldb: LightDataBuilder = edb.into();
        assert!(ldb.get_energy_distribution_type().is_none());
        let ldb: LightDataBuilder = PointSrc::default().into();
        assert!(ldb.get_energy_distribution_type().is_some());
    }
    #[test]
    fn get_position_distribution_type() {
        let edb = EnergyDataBuilder::default();
        let ldb: LightDataBuilder = edb.into();
        assert!(ldb.get_position_distribution_type().is_none());
        let ldb: LightDataBuilder = PointSrc::default().into();
        assert!(ldb.get_position_distribution_type().is_some());
    }
    #[test]
    fn get_spectral_distribution_type() {
        let edb = EnergyDataBuilder::default();
        let ldb: LightDataBuilder = edb.into();
        assert!(ldb.get_spectral_distribution_type().is_none());
        let ldb: LightDataBuilder = PointSrc::default().into();
        assert!(ldb.get_spectral_distribution_type().is_some());
    }
    #[test]
    fn from_energy_data_builder() {
        let ldb: LightDataBuilder = EnergyDataBuilder::default().into();
        assert!(matches!(ldb, LightDataBuilder::Energy(_)));
    }
    #[test]
    fn from_img_src() {
        let ldb: LightDataBuilder = ImageSrc::default().into();
        assert!(matches!(ldb, LightDataBuilder::Geometric(_)));
    }
    #[test]
    fn from_point_src() {
        let ldb: LightDataBuilder = PointSrc::default().into();
        assert!(matches!(ldb, LightDataBuilder::Geometric(_)));
    }
    #[test]
    fn from_collimated_src() {
        let ldb: LightDataBuilder = CollimatedSrc::default().into();
        assert!(matches!(ldb, LightDataBuilder::Geometric(_)));
    }
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
        let light_data_builder = LightDataBuilder::Geometric(RayDataSource::Raw(Rays::default()));
        let light_data = light_data_builder.build().unwrap();
        assert!(matches!(light_data, LightData::Geometric(_)));
    }
}
