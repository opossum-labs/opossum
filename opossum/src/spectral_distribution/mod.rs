//! Module for handling spectral distributions
use crate::error::OpmResult;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}
