//! Thermal properties module for materials
use serde::{Deserialize, Serialize};
use uom::si::f64::{TemperatureCoefficient, ThermalConductivity};

/// Optional thermal properties of a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ThermalProperties {
    #[serde(default)]
    thermal_conductivity: Option<ThermalConductivity>,

    /// Coefficient of thermal expansion
    #[serde(default)]
    expansion_coefficient: Option<TemperatureCoefficient>,
}

impl ThermalProperties {
    /// Creates a new `ThermalProperties` instance.
    #[must_use]
    pub const fn new(
        thermal_conductivity: Option<ThermalConductivity>,
        expansion_coefficient: Option<TemperatureCoefficient>,
    ) -> Self {
        Self {
            thermal_conductivity,
            expansion_coefficient,
        }
    }
}
