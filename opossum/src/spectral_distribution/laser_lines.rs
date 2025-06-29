use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::error::{OpmResult, OpossumError};

use super::SpectralDistribution;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// A struct representing a collection of laser lines with their respective wavelengths and relative intensities.
pub struct LaserLines {
    lines: Vec<(Length, f64)>,
}
impl LaserLines {
    /// Creates a new `LaserLines` instance with the given laser lines.
    ///
    /// The given intensities are normalized to sum to 1.0.
    ///
    /// # Arguments
    ///
    /// * `lines` - A vector of tuples containing the wavelength and intensity of each laser line.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// * the vector is empty,
    /// * any wavelength is negative or infinite,
    /// * any intensity is negative or infinite,
    /// * the sum of intensities is zero.
    pub fn new(lines: Vec<(Length, f64)>) -> OpmResult<Self> {
        // Check if the lines are non-empty and contain valid data
        if lines.is_empty() {
            return Err(OpossumError::Other("Laser lines cannot be empty".into()));
        }
        for (wavelength, intensity) in &lines {
            if !wavelength.is_normal() || wavelength.is_sign_negative() {
                return Err(OpossumError::Other(
                    "Wavelength must be positive and finite".into(),
                ));
            }
            if !intensity.is_normal() || intensity.is_sign_negative() {
                return Err(OpossumError::Other(
                    "Intensity must be positive and finite".into(),
                ));
            }
        }
        // Normalize the intensities to sum to 1.0
        let sum_intensity: f64 = lines.iter().map(|(_, intensity)| *intensity).sum();
        if sum_intensity == 0.0 {
            return Err(OpossumError::Other(
                "Sum of intensities cannot be zero".into(),
            ));
        }
        let lines: Vec<(Length, f64)> = lines
            .into_iter()
            .map(|(wavelength, intensity)| (wavelength, intensity / sum_intensity))
            .collect();
        Ok(Self { lines })
    }
}
impl SpectralDistribution for LaserLines {
    /// Generates the laser lines.
    ///
    /// # Returns
    ///
    /// A vector of tuples containing the wavelength and intensity of each laser line.
    fn generate(&self) -> OpmResult<Vec<(Length, f64)>> {
        Ok(self.lines.clone())
    }
}
impl From<LaserLines> for super::SpecDistType {
    fn from(laser_lines: LaserLines) -> Self {
        Self::LaserLines(laser_lines)
    }
}
