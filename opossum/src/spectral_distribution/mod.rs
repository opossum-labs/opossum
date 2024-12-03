//! Module for handling spectral distributions
use crate::error::OpmResult;
use uom::si::f64::Length;

pub mod gaussian;
pub use gaussian::Gaussian;

pub trait SpectralDistribution {
    /// Creates a Gaussian spectral distribution
    /// # Errors
    /// This function only propagates errors of the contained functions
    fn generate(&self) -> OpmResult<Vec<(Length, f64)>>;
}
