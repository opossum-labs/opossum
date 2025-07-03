//! Module for handling the refractive index of an optical material.
#![warn(missing_docs)]
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum::EnumIter;
use strum::IntoEnumIterator;
use uom::si::f64::Length;

pub mod refr_index_conrady;
pub mod refr_index_const;
pub mod refr_index_schott;
pub mod refr_index_sellmeier1;

use self::refr_index_schott::RefrIndexSchott;
pub use refr_index_conrady::RefrIndexConrady;
pub use refr_index_const::RefrIndexConst;
pub use refr_index_const::refr_index_vaccuum;
pub use refr_index_sellmeier1::RefrIndexSellmeier1;

use crate::error::{OpmResult, OpossumError};
use crate::properties::Proptype;

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
}

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
        };
        if refr_index < 1.0 || !refr_index.is_finite() {
            return Err(OpossumError::Other(
                "refractive index calculated by model is <1.0 or not finite".into(),
            ));
        }
        Ok(refr_index)
    }

    /// Creates a default instance of a Refractive index type by name.
    ///
    /// This is used to instantiate a predefined refractive index type from a string input,
    /// e.g., in configuration files or UI selections.
    ///
    /// # Parameters
    /// - `name`: The name of the desired refractive index type.
    ///
    /// # Returns
    /// - `Some(RefractiveIndexType)` if the name is recognized.
    /// - `None` if the name is unknown.
    #[must_use]
    pub fn default_from_name(name: &str) -> Option<Self> {
        Self::iter().find(|ref_ind_type| format!("{ref_ind_type}") == name)
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
    /// Create a corresponding [`RefractiveIndexType`] value.
    ///
    /// This function is mainly used to store a model in a [`Property`](crate::properties::property::Property)
    fn to_enum(&self) -> RefractiveIndexType;
}
