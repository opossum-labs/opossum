//! Builder for the generation of [`LightData`].
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use std::fmt::Display;

use crate::{
    energy_distributions::EnergyDistType, error::OpmResult, position_distributions::PosDistType,
    rays::Rays,
};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use super::LightData;

/// Builder for the generation of [`LightData::Geometric`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RayDataBuilder {
    /// Raw [`Rays`] data.
    Raw(Rays),
    /// Collimated [`Rays`] data with a given [`PosDistType`] and [`EnergyDistType`] as well as a given wavelength.
    Collimated(PosDistType, EnergyDistType, Length),
}

impl RayDataBuilder {
    /// Create [`LightData::Geometric`] from the builder definition.
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the builder fails.
    pub fn build(self) -> OpmResult<LightData> {
        match self {
            Self::Raw(rays) => Ok(LightData::Geometric(rays)),
            Self::Collimated(pos_dist, energy_dist, wave_length) => {
                let rays =
                    Rays::new_collimated(wave_length, energy_dist.generate(), pos_dist.generate())?;
                Ok(LightData::Geometric(rays))
            }
        }
    }
}

impl Display for RayDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(r) => write!(f, "Raw({r})"),
            Self::Collimated(pos_dist, energy_dist, wave_length) => {
                write!(
                    f,
                    "Collimated({pos_dist:?}, {energy_dist:?}, {wave_length:?})"
                )
            }
        }
    }
}
