//! Builder for [`LightData`]
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use super::{
    energy_spectrum_builder::EnergyDataBuilder, ray_data_builder::RayDataBuilder, LightData,
};
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
