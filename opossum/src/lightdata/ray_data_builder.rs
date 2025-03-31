//! Builder for the generation of [`LightData`].
//!
//! This module provides a builder for the generation of [`LightData`] to be used in `Source`.
//! This builder allows easier serialization / deserialization in OPM files.
use std::fmt::Display;

use super::LightData;
use crate::{
    energy_distributions::EnergyDistType, error::OpmResult, position_distributions::PosDistType,
    rays::Rays, spectral_distribution::SpecDistType,
};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};

/// Builder for the generation of [`LightData::Geometric`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RayDataBuilder {
    /// Raw [`Rays`] data.
    Raw(Rays),
    /// Collimated [`Rays`] data with a given [`PosDistType`], [`EnergyDistType`], and [`SpecDistType`].
    Collimated {
        /// Position distribution.
        pos_dist: PosDistType,
        /// Energy distribution.
        energy_dist: EnergyDistType,
        /// Wavelength of the rays.
        spect_dist: SpecDistType,
    },
    /// Point source [`Rays`] data with a given [`PosDistType`], [`EnergyDistType`], and [`SpecDistType`].
    /// All rays start on the optical axis and are emitted within a cone. The cone is defined by the
    /// position distribution **after the rays have propagated the given reference length**.
    PointSrc {
        /// Position distribution.
        pos_dist: PosDistType,
        /// Energy distribution.
        energy_dist: EnergyDistType,
        /// Wavelength of the rays.
        spect_dist: SpecDistType,
        /// Length
        reference_length: Length,
    },
}

impl RayDataBuilder {
    /// Create [`LightData::Geometric`] from the builder definition.
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the builder fails.
    pub fn build(self) -> OpmResult<LightData> {
        match self {
            Self::Raw(rays) => Ok(LightData::Geometric(rays)),
            Self::Collimated {
                pos_dist,
                energy_dist,
                spect_dist,
            } => {
                let rays = Rays::new_collimated_with_spectrum(
                    spect_dist.generate(),
                    energy_dist.generate(),
                    pos_dist.generate(),
                )?;
                Ok(LightData::Geometric(rays))
            }
            Self::PointSrc {
                pos_dist,
                energy_dist,
                spect_dist,
                reference_length,
            } => {
                let rays = Rays::new_point_src_with_spectrum(
                    spect_dist.generate(),
                    energy_dist.generate(),
                    pos_dist.generate(),
                    reference_length,
                )?;
                Ok(LightData::Geometric(rays))
            }
        }
    }
}

impl Display for RayDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(r) => write!(f, "Raw({r})"),
            Self::Collimated {
                pos_dist,
                energy_dist,
                spect_dist,
            } => {
                write!(
                    f,
                    "Collimated({pos_dist:?}, {energy_dist:?}, {spect_dist:?})"
                )
            }
            Self::PointSrc {
                pos_dist,
                energy_dist,
                spect_dist,
                reference_length,
            } => {
                write!(
                    f,
                    "PointSrc({pos_dist:?}, {energy_dist:?}, {spect_dist:?}, {}m)",
                    reference_length.get::<meter>()
                )
            }
        }
    }
}
