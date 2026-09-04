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
#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;

    #[test]
    fn test_thermal_properties_new_and_default() {
        // Verify default initialization
        let default_props = ThermalProperties::default();
        assert_eq!(default_props.thermal_conductivity, None);
        assert_eq!(default_props.expansion_coefficient, None);

        // Verify constructor with explicit values
        let conductivity = ThermalConductivity::new::<watt_per_meter_kelvin>(1.4);
        let expansion = TemperatureCoefficient::new::<per_kelvin>(7.1e-6);
        let props = ThermalProperties::new(Some(conductivity), Some(expansion));

        assert_eq!(props.thermal_conductivity, Some(conductivity));
        assert_eq!(props.expansion_coefficient, Some(expansion));
    }

    #[test]
    fn test_thermal_properties_serde_roundtrip() {
        let conductivity = ThermalConductivity::new::<watt_per_meter_kelvin>(1.4);
        let expansion = TemperatureCoefficient::new::<per_kelvin>(7.1e-6);
        let props = ThermalProperties::new(Some(conductivity), Some(expansion));

        let ron = ron::to_string(&props).expect("serialization failed");
        let deserialized: ThermalProperties = ron::from_str(&ron).expect("deserialization failed");

        assert_eq!(props, deserialized);
    }
}
