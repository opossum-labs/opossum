//! Builder for [`LightData`]
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use super::{LightData, energy_data_builder::EnergyDataBuilder, ray_data_builder::RayDataBuilder};
use crate::{error::OpmResult, properties::Proptype};

/// Builder for the generation of [`LightData`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LightDataBuilder {
    /// Builder for the generation of [`LightData::Energy`].
    Energy(EnergyDataBuilder),
    /// Builder for the generation of [`LightData::Geometric`].
    Geometric(RayDataBuilder),
    /// Dummy Fourier
    Fourier,
}

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
            Self::Fourier => Ok(LightData::Fourier),
        }
    }
}

impl Display for LightDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Energy(e) => write!(f, "Energy({e})"),
            Self::Geometric(r) => write!(f, "Geometric({r})"),
            Self::Fourier => write!(f, "Fourier"),
        }
    }
}
impl From<Option<LightDataBuilder>> for Proptype {
    fn from(value: Option<LightDataBuilder>) -> Self {
        Self::LightDataBuilder(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{joule, nanometer, rays::Rays};

    #[test]
    fn from_light_data_builder_to_proptype() {
        let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            vec![(nanometer!(1000.0), joule!(1.0))],
            nanometer!(1.0),
        ));
        let proptype: Proptype = Some(light_data_builder).into();
        assert!(matches!(proptype, Proptype::LightDataBuilder(_)));
    }
    #[test]
    fn display_light_data_builder() {
        let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            vec![(nanometer!(1000.0), joule!(1.0))],
            nanometer!(1.0),
        ));
        assert_eq!(
            format!("{light_data_builder}"),
            "Energy(LaserLines([(1.0000000000000002e-6 m^1, 1.0 m^2 kg^1 s^-2)], 1.000 nm))"
        );
    }
    #[test]
    fn build_light_data() {
        let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            vec![(nanometer!(1000.0), joule!(1.0))],
            nanometer!(1.0),
        ));
        let light_data = light_data_builder.build().unwrap();
        assert!(matches!(light_data, LightData::Energy(_)));
        let light_data_builder = LightDataBuilder::Fourier;
        let light_data = light_data_builder.build().unwrap();
        assert!(matches!(light_data, LightData::Fourier));
        let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Raw(Rays::default()));
        let light_data = light_data_builder.build().unwrap();
        assert!(matches!(light_data, LightData::Geometric(_)));
    }
}
