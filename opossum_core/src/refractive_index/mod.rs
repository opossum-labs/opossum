//! Module for handling the refractive index of an optical material.
#![warn(missing_docs)]
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;
use uom::si::f64::Length;

pub mod bounded_model;
pub mod ref_index_air;
pub mod refr_index_conrady;
pub mod refr_index_const;
pub mod refr_index_schott;
pub mod refr_index_sellmeier1;

pub use bounded_model::{BoundedFormula, DispersionFormula};
pub use ref_index_air::RefrIndexAir;
pub use refr_index_conrady::RefrIndexConrady;
pub use refr_index_const::{RefrIndexConst, refr_index_vaccuum};
pub use refr_index_schott::RefrIndexSchott;
pub use refr_index_sellmeier1::RefrIndexSellmeier1;

use crate::{
    error::{OpmResult, OpossumError},
    properties::Proptype,
    utils::default_from_name::DefaultFromName,
};

/// Available models for the calculation of refractive index
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, EnumIter)]
pub enum RefractiveIndexType {
    /// Trivial model returning a wavelength-independant constant
    Const(RefrIndexConst),
    /// Sellmeier 1 model
    Sellmeier1(RefrIndexSellmeier1),
    /// Schott model
    Schott(RefrIndexSchott),
    /// Conrady model
    Conrady(RefrIndexConrady),
    /// Air model (Edlén formula modified by Birch and Downs)
    Air(RefrIndexAir),
}

impl DefaultFromName for RefractiveIndexType {}

impl Default for RefractiveIndexType {
    fn default() -> Self {
        Self::Sellmeier1(RefrIndexSellmeier1::default())
    }
}

impl RefractiveIndexType {
    /// Get the refractive index value of the [`RefractiveIndexType`] for the given wavelength.
    ///
    /// # Errors
    ///
    /// This function returns an error if the the refractive index could not be calculated e.g.:
    ///   - the given wavelength is outside defined limits.
    ///   - the model would calculate a value below 1.0, NaN or infinity
    pub fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        let refr_index = match self {
            Self::Const(refr_index_const) => refr_index_const.get_refractive_index(wavelength)?,
            Self::Sellmeier1(refr_index_sellmeier1) => {
                refr_index_sellmeier1.get_refractive_index(wavelength)?
            }
            Self::Schott(refr_index_schott) => {
                refr_index_schott.get_refractive_index(wavelength)?
            }
            Self::Conrady(refr_index_conrady) => {
                refr_index_conrady.get_refractive_index(wavelength)?
            }
            Self::Air(refr_index_air) => refr_index_air.get_refractive_index(wavelength)?,
        };
        if refr_index < 1.0 || !refr_index.is_finite() {
            return Err(OpossumError::Other(
                "refractive index calculated by model is <1.0 or not finite".into(),
            ));
        }
        Ok(refr_index)
    }
}

impl From<RefractiveIndexType> for Proptype {
    fn from(refr: RefractiveIndexType) -> Self {
        Self::RefractiveIndex(refr)
    }
}

impl Display for RefractiveIndexType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Const(_) => write!(f, "Constant"),
            Self::Sellmeier1(_) => write!(f, "Sellmeier equation"),
            Self::Schott(_) => write!(f, "Schott equation"),
            Self::Conrady(_) => write!(f, "Conrady equation"),
            Self::Air(_) => write!(f, "Air model"),
        }
    }
}
/// All refractive index models must implement this trait.
pub trait RefractiveIndex {
    /// Get the refractive index value of the current model for the given wavelength.
    ///
    /// # Errors
    ///
    /// This function returns an error if the the refractive index could not be calculated e.g.:
    ///   - the given wavelength is outside defined limits.
    ///   - the model would calculate a value below 1.0, NaN or infinity
    fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64>;
}

impl From<&RefrIndexConst> for RefractiveIndexType {
    fn from(val: &RefrIndexConst) -> Self {
        Self::Const(*val)
    }
}

impl From<&RefrIndexSellmeier1> for RefractiveIndexType {
    fn from(val: &RefrIndexSellmeier1) -> Self {
        Self::Sellmeier1(val.clone())
    }
}

impl From<&RefrIndexSchott> for RefractiveIndexType {
    fn from(val: &RefrIndexSchott) -> Self {
        Self::Schott(val.clone())
    }
}

impl From<&RefrIndexConrady> for RefractiveIndexType {
    fn from(val: &RefrIndexConrady) -> Self {
        Self::Conrady(val.clone())
    }
}

impl From<&RefrIndexAir> for RefractiveIndexType {
    fn from(val: &RefrIndexAir) -> Self {
        Self::Air(val.clone())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::nanometer;

    #[test]
    fn test_default_is_sellmeier() {
        let default_idx = RefractiveIndexType::default();
        // The default implementation should return Sellmeier1.
        assert!(matches!(default_idx, RefractiveIndexType::Sellmeier1(_)));
    }

    #[test]
    fn test_display_strings() -> OpmResult<()> {
        // Test that each variant displays the correct descriptive string.
        assert_eq!(
            format!("{}", RefractiveIndexType::Const(RefrIndexConst::new(1.5)?)),
            "Constant"
        );
        assert_eq!(
            format!(
                "{}",
                RefractiveIndexType::Sellmeier1(RefrIndexSellmeier1::default())
            ),
            "Sellmeier equation"
        );
        assert_eq!(
            format!(
                "{}",
                RefractiveIndexType::Schott(RefrIndexSchott::default())
            ),
            "Schott equation"
        );
        assert_eq!(
            format!(
                "{}",
                RefractiveIndexType::Conrady(RefrIndexConrady::default())
            ),
            "Conrady equation"
        );
        Ok(())
    }

    #[test]
    fn test_central_validation_logic() -> OpmResult<()> {
        // Case 1: Valid calculation (N-BK7 default at 1050nm).
        let refr = RefractiveIndexType::default();
        let n = refr.get_refractive_index(nanometer!(1050.0))?;
        assert!(n >= 1.0);

        // Case 2: Validation during construction
        // We verify that RefrIndexConst prevents invalid values at the start.
        assert!(RefrIndexConst::new(0.9).is_err());
        assert!(RefrIndexConst::new(f64::NAN).is_err());

        // Case 3: The "Safety Net" in RefractiveIndexType
        // We create a Sellmeier model that is valid at construction (finite coefficients)
        // but produces a value < 1.0 or NaN during calculation.
        // Sellmeier1 allows negative k-coefficients.
        let sneaky_sellmeier = RefrIndexSellmeier1::new(
            -10.0,
            0.0,
            0.0, // Large negative k1
            0.01,
            0.02,
            103.0,
            nanometer!(1000.0)..nanometer!(1100.0),
        )?;

        let sneaky_enum = RefractiveIndexType::Sellmeier1(sneaky_sellmeier);
        let result = sneaky_enum.get_refractive_index(nanometer!(1050.0));

        // This must be caught by the central check in RefractiveIndexType.
        assert!(result.is_err());
        if let Err(OpossumError::Other(msg)) = result {
            assert!(msg.contains("<1.0 or not finite"));
        }
        Ok(())
    }

    #[test]
    fn test_error_propagation_from_variants() {
        // Ensure that variant-specific errors (like out of range) are propagated.
        let bk7 = RefractiveIndexType::default(); // Valid range: 300..2500nm
        let out_of_range = bk7.get_refractive_index(nanometer!(2500.0));
        assert!(out_of_range.is_err());
    }

    #[test]
    fn test_trait_from_trait_consistency() {
        // Ensure the RefractiveIndex trait's to_enum works for the variants.
        let schott = RefrIndexSchott::default();
        let enu = RefractiveIndexType::from(&schott);
        assert!(matches!(enu, RefractiveIndexType::Schott(_)));
    }
}
