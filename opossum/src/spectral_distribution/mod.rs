//! Module for handling spectral distributions
use std::fmt::Display;

use crate::error::OpmResult;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;
use strum::EnumIter;

pub mod gaussian;
pub mod laser_lines;
pub use gaussian::Gaussian;
pub use laser_lines::LaserLines;

pub trait SpectralDistribution {
    /// Creates a Gaussian spectral distribution
    /// # Errors
    /// This function only propagates errors of the contained functions
    fn generate(&self) -> OpmResult<Vec<(Length, f64)>>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumIter)]
/// Enum representing different types of spectral distributions
pub enum SpecDistType {
    Gaussian(gaussian::Gaussian),
    LaserLines(laser_lines::LaserLines),
}
impl SpecDistType {
    /// Generates the spectral distribution
    /// # Errors
    /// This function only propagates errors of the contained functions
    #[must_use]
    pub fn generate(&self) -> &dyn SpectralDistribution {
        match self {
            Self::Gaussian(g) => g,
            Self::LaserLines(l) => l,
        }
    }

    pub fn default_from_name(name: &str) -> Option<Self> {
        match name {
            "Laser Lines" => Some(LaserLines::new_empty().into()),
            "Gaussian" => Some(Gaussian::default().into()),
            _ => None,
        }
    }
}

impl Default for SpecDistType {
    fn default() -> Self {
        Self::Gaussian(Gaussian::default())
    }
}

impl Display for SpecDistType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dist_string = match self {
            Self::LaserLines(_) => "Laser Lines",
            Self::Gaussian(_) => "Gaussian"            
        };
        write!(f, "{dist_string}")
    }
}
