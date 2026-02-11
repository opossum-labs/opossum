//! Generic bounded refractive index model implementation.

use std::ops::Range;

use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::error::{OpmResult, OpossumError};

use super::RefractiveIndex;

/// Trait for the core mathematical dispersion formula.
///
/// Implementors of this trait only need to provide the raw calculation.
/// Validation ranges and unit handling are managed by [`BoundedFormula`].
pub trait DispersionFormula:
    Clone + Serialize + for<'de> Deserialize<'de> + PartialEq + core::fmt::Debug
{
    /// Calculate the refractive index for a given wavelength.
    ///
    /// The input wavelength is guaranteed to be within the valid range defined in the wrapper.
    /// Calculate the refractive index for a given wavelength.
    ///
    /// The input wavelength is guaranteed to be within the valid range defined in the wrapper.
    fn calculate(&self, wavelength: Length) -> f64;
}

/// A generic wrapper that adds wavelength range validation to any [`DispersionFormula`].
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct BoundedFormula<T> {
    /// The coefficients/parameters of the dispersion formula.
    pub coefficients: T,
    /// The valid wavelength range.
    pub wvl_range: Range<Length>,
}

impl<T: DispersionFormula> BoundedFormula<T> {
    /// Create a new bounded dispersion model.
    ///
    /// # Errors
    ///
    /// Returns an error if the wavelength range is invalid (start < 0, start >= end).
    /// Note: This constructor does NOT validate the coefficients themselves, as their
    /// validity depends on the specific formula logic. The `coefficients` object is
    /// assumed to be valid at this point.
    /// Create a new bounded dispersion model from coefficients and range.
    pub fn from_coefficients(coefficients: T, wavelength_range: Range<Length>) -> OpmResult<Self> {
        if wavelength_range.start.is_sign_negative() || !wavelength_range.start.is_finite() {
            return Err(OpossumError::Other(
                "lower wavelength limit is invalid.".into(),
            ));
        }
        if wavelength_range.end.is_sign_negative() || !wavelength_range.end.is_finite() {
            return Err(OpossumError::Other(
                "upper wavelength limit is invalid.".into(),
            ));
        }
        if wavelength_range.start >= wavelength_range.end {
            return Err(OpossumError::Other(
                "wavelength range start must be less than end".into(),
            ));
        }

        Ok(Self {
            coefficients,
            wvl_range: wavelength_range,
        })
    }

    /// Returns a reference to the inner coefficients.
    pub const fn coefficients(&self) -> &T {
        &self.coefficients
    }

    /// Returns a mutable reference to the inner coefficients.
    pub const fn coefficients_mut(&mut self) -> &mut T {
        &mut self.coefficients
    }

    /// Returns the valid wavelength range.
    pub const fn wavelength_range(&self) -> &Range<Length> {
        &self.wvl_range
    }

    /// Sets the valid wavelength range.
    pub fn set_wavelength_range(&mut self, range: Range<Length>) {
        self.wvl_range = range;
    }

    /// Sets the start of the valid wavelength range.
    pub fn set_wavelength_range_start(&mut self, start: Length) {
        self.wvl_range.start = start;
    }

    /// Sets the end of the valid wavelength range.
    pub fn set_wavelength_range_end(&mut self, end: Length) {
        self.wvl_range.end = end;
    }
}

impl<T: DispersionFormula> RefractiveIndex for BoundedFormula<T> {
    fn get_refractive_index(&self, wavelength: Length) -> OpmResult<f64> {
        if !self.wvl_range.contains(&wavelength) {
            return Err(OpossumError::Other("wavelength outside valid range".into()));
        }

        let n = self.coefficients.calculate(wavelength);

        // Universal sanity check for all refractive indices
        if n < 1.0 || !n.is_finite() {
            return Err(OpossumError::Other(
                "refractive index calculated by model is <1.0 or not finite".into(),
            ));
        }

        Ok(n)
    }
}
