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
#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::pascal;

    #[test]
    fn test_mechanical_properties_new_and_default() {
        // Verify default initializes all optional fields to None
        let default_props = MechanicalProperties::default();
        assert_eq!(default_props.density, None);
        assert_eq!(default_props.youngs_modulus, None);

        // Verify constructor properly sets provided quantities
        let density = MassDensity::new::<kilogram_per_cubic_meter>(2500.0);
        let modulus = Pressure::new::<pascal>(70e9);
        let props = MechanicalProperties::new(Some(density), Some(modulus));

        assert_eq!(props.density, Some(density));
        assert_eq!(props.youngs_modulus, Some(modulus));
    }

    #[test]
    fn test_mechanical_properties_serde_roundtrip() {
        let density = MassDensity::new::<kilogram_per_cubic_meter>(2200.0);
        let modulus = Pressure::new::<pascal>(65e9);
        let props = MechanicalProperties::new(Some(density), Some(modulus));

        let ron = ron::to_string(&props).expect("serialization failed");
        let deserialized: MechanicalProperties =
            ron::from_str(&ron).expect("deserialization failed");

        assert_eq!(props, deserialized);
    }
}
