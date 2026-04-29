mod band;
mod edge;
mod math;
mod spectral_builder;

pub use band::{BandFilter, BandFilterType};
pub use edge::{EdgeFilter, EdgeFilterType};
pub use spectral_builder::SpectralFilterBuilder;

use std::fmt::Display;

use crate::{error::OpmResult, light::Spectrum, utils::default_from_name::DefaultFromName};
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

/// Config data builder for an [`IdealFilter`](crate::nodes::IdealFilter).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum FilterTypeBuilder {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(f64),
    /// filter based on given transmission spectrum.
    Spectrum(SpectralFilterBuilder),
}

impl Display for FilterTypeBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Constant(_) => write!(f, "Constant"),
            Self::Spectrum(_) => write!(f, "Spectral filter"),
        }
    }
}

impl FilterTypeBuilder {
    /// Constructs a [`FilterType`] object from the builder.
    ///
    /// # Returns
    /// - A [`FilterType`] instance corresponding to the variant
    /// # Errors
    /// Returns an error if the creation of a spectrum from a .csv fails.
    pub fn build(&self) -> OpmResult<FilterType> {
        match self {
            Self::Constant(c) => Ok(FilterType::Constant(*c)),
            Self::Spectrum(spectral_filter_builder) => {
                Ok(FilterType::Spectrum(spectral_filter_builder.build()?))
            }
        }
    }
}

impl DefaultFromName for FilterTypeBuilder {
    fn default_from_name(name: &str) -> Option<Self> {
        for ftb in Self::iter() {
            if name == format!("{ftb}") {
                match ftb {
                    Self::Constant(_) => {
                        return Some(Self::Constant(1.0));
                    }
                    Self::Spectrum(_) => return Some(ftb),
                }
            }
        }
        None
    }
}

/// Config data for an [`IdealFilter`](crate::nodes::IdealFilter).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilterType {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(f64),
    /// filter based on given transmission spectrum.
    Spectrum(Spectrum),
}

impl From<f64> for FilterTypeBuilder {
    fn from(val: f64) -> Self {
        Self::Constant(val)
    }
}
