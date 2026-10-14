//! Optical properties module for materials

use serde::{Deserialize, Serialize};

use crate::{absorption::absorption_model::AbsorptionModel, refractive_index::RefractiveIndexType};

/// Primary optical properties required for optical simulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpticalProperties {
    /// Refractive index calculation model.
    pub refractive_index: RefractiveIndexType,

    /// Absorption model specifying transmittance and attenuation.
    #[serde(default, skip_serializing_if = "AbsorptionModel::is_none")]
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        absorption::absorption_constant::AbsConst, error::OpmResult,
        refractive_index::RefrIndexConst,
    };

    #[test]
    fn test_optical_properties_constructors() -> OpmResult<()> {
        let const_refr = RefrIndexConst::new(1.5)?;
        let props = OpticalProperties::new(const_refr.into());
        assert_eq!(
            props.absorption,
            crate::absorption::absorption_model::AbsorptionModel::default()
        );

        // Test constructor with explicit absorption model
        let custom_absorption = crate::absorption::absorption_model::AbsorptionModel::default();
        let custom_props = OpticalProperties::with_absorption(
            RefrIndexConst::new(1.6)?.into(),
            custom_absorption.clone(),
        );
        assert_eq!(custom_props.absorption, custom_absorption);
        Ok(())
    }
    #[test]
    fn test_optical_properties_serde_roundtrip() -> OpmResult<()> {
        let const_refr = RefrIndexConst::new(1.5)?;
        let props = OpticalProperties::new(const_refr.into());

        let ron = ron::to_string(&props).expect("serialization failed");
        let deserialized: OpticalProperties = ron::from_str(&ron).expect("deserialization failed");

        assert_eq!(props, deserialized);
        Ok(())
    }
    #[test]
    fn test_optical_properties_serde_skips_none_absorption() -> OpmResult<()> {
        let const_refr = RefrIndexConst::new(1.5)?;
        let props = OpticalProperties::new(const_refr.into());

        // Serialization should omit 'absorption'
        let ron_str = ron::to_string(&props).expect("serialization failed");
        assert!(!ron_str.contains("absorption"));

        // Deserialization reconstructs AbsorptionModel::None via serde default
        let deserialized: OpticalProperties =
            ron::from_str(&ron_str).expect("deserialization failed");
        assert_eq!(props, deserialized);
        assert_eq!(deserialized.absorption, AbsorptionModel::None);
        Ok(())
    }
    #[test]
    fn test_optical_properties_serde_serializes_active_absorption() -> OpmResult<()> {
        let const_refr = RefrIndexConst::new(1.5)?;
        let custom_abs = AbsorptionModel::ConstantAttenuation(AbsConst::new(0.9)?);
        let props = OpticalProperties::with_absorption(const_refr.into(), custom_abs);

        let ron_str = ron::to_string(&props).expect("serialization failed");
        assert!(ron_str.contains("absorption"));

        let deserialized: OpticalProperties =
            ron::from_str(&ron_str).expect("deserialization failed");
        assert_eq!(props, deserialized);
        Ok(())
    }
}
