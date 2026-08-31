//! Optical properties module for materials

use serde::{Deserialize, Serialize};

use crate::{absorption::absorption_model::AbsorptionModel, refractive_index::RefractiveIndexType};

/// Primary optical properties required for optical simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticalProperties {
    /// Refractive index calculation model.
    pub refractive_index: RefractiveIndexType,

    /// Absorption model specifying transmittance and attenuation.
    #[serde(default)]
    pub absorption: AbsorptionModel,
}

impl OpticalProperties {
    /// Creates a new `OpticalProperties` container with default absorption.
    #[must_use]
    pub fn new(refractive_index: RefractiveIndexType) -> Self {
        Self {
            refractive_index,
            absorption: AbsorptionModel::default(),
        }
    }

    /// Creates a container with a custom refractive index and custom absorption model.
    #[must_use]
    pub const fn with_absorption(
        refractive_index: RefractiveIndexType,
        absorption: AbsorptionModel,
    ) -> Self {
        Self {
            refractive_index,
            absorption,
        }
    }
}
