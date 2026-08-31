//! Mechanical properties module for materials

use serde::{Deserialize, Serialize};
use uom::si::f64::{MassDensity, Pressure};

/// Optional mechanical properties of a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MechanicalProperties {
    /// Mass density
    #[serde(default)]
    density: Option<MassDensity>,

    /// Young's modulus
    #[serde(default)]
    youngs_modulus: Option<Pressure>,
}

impl MechanicalProperties {
    /// Creates a new `MechanicalProperties` instance.
    #[must_use]
    pub const fn new(density: Option<MassDensity>, youngs_modulus: Option<Pressure>) -> Self {
        Self {
            density,
            youngs_modulus,
        }
    }
}
