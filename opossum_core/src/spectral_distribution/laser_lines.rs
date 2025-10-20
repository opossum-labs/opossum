use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{AllNormal, AllNotEmpty, AllPositive},
    nanometer, validated, validated_type,
};

use super::SpectralDistribution;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// A struct representing a collection of laser lines with their respective wavelengths and relative intensities.
pub struct LaserLines {
    lines: validated_type!(
        Vec<(
            validated_type!(Length, AllNormal && AllPositive),
            validated_type!(f64, AllNormal && AllPositive)
        )>,
        AllNotEmpty
    ),
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
        let mut laser_lines = Self::default();
        laser_lines.add_lines(lines)?;
        Ok(laser_lines)
    }

    /// Adds a list of laser lines to the [`LaserLines`] distribution.
    ///
    /// Each laser line is a tuple containing a [`Length`] representing the wavelength,
    /// and a `f64` representing the intensity.
    ///
    /// # Parameters
    /// * `lines` – A vector of `(Length, f64)` tuples, each representing a spectral line.
    ///
    /// # Returns
    /// * `Ok(())` if all lines are valid and added successfully.
    /// * `Err(OpossumError)` if validation fails.
    ///
    /// # Errors
    /// This method returns an error if:
    /// - The input list is empty.
    /// - Any wavelength is negative or not finite.
    /// - Any intensity is negative or not finite.
    pub fn add_lines(&mut self, lines: Vec<(Length, f64)>) -> OpmResult<()> {
        let mut validated_vec = self.lines.get().clone();
        for (wvl, intensity) in lines {
            validated_vec.push((
                validated!(wvl, AllNormal && AllPositive)?,
                validated!(intensity, AllNormal && AllPositive)?,
            ));
        }
        self.lines.set(validated_vec)?;
        Ok(())
    }

    /// Returns an immutable reference to the list of laser lines stored in this [`LaserLines`] instance.
    ///
    /// Each line is represented as a tuple `(Length, f64)`, where the `Length` is the wavelength and
    /// `f64` is the corresponding intensity.
    ///
    /// # Returns
    /// A reference to the vector of spectral lines.

    #[must_use]
    pub fn lines(&self) -> Vec<(&Length, &f64)> {
        // &Vec<(validated_type!(Length, AllNormal && AllPositive), validated_type!(f64, AllNormal && AllPositive))> {
        self.lines
            .get()
            .iter()
            .map(|l| (l.0.get(), l.1.get()))
            .collect::<Vec<(&Length, &f64)>>()
    }

    /// Deletes a line form `LaserLines`
    /// 
    /// # Errors
    /// Returns an error if setting the new lines fails. More of an esotheric error that should not happen as the other lines have all been validated before
    pub fn delete_line(&mut self, index: usize) -> OpmResult<()> {
        let lines = self.lines.get();
        if index < lines.len() {
            let mut lines = lines.clone();
            lines.remove(index);
            self.lines.set(lines)?;
        }
        Ok(())
    }
}

impl Default for LaserLines {
    fn default() -> Self {
        let validated_length = validated!(nanometer!(1054.), AllNormal && AllPositive).unwrap();
        let validated_intensity = validated!(1., AllNormal && AllPositive).unwrap();

        Self {
            lines: validated!(vec![(validated_length, validated_intensity)], AllNotEmpty).unwrap(),
        }
    }
}

impl SpectralDistribution for LaserLines {
    /// Generates the laser lines.
    ///
    /// # Returns
    ///
    /// A vector of tuples containing the wavelength and intensity of each laser line.
    fn generate(&self) -> OpmResult<Vec<(Length, f64)>> {
        // Normalize the intensities to sum to 1.0
        let sum_intensity: f64 = self
            .lines
            .get()
            .iter()
            .map(|(_, intensity)| *intensity.get())
            .sum();
        if sum_intensity == 0.0 {
            return Err(OpossumError::Other(
                "Sum of intensities cannot be zero".into(),
            ));
        }
        let lines: Vec<(Length, f64)> = self
            .lines
            .get()
            .iter()
            .map(|(wavelength, intensity)| (*wavelength.get(), *intensity.get() / sum_intensity))
            .collect();
        Ok(lines)
    }
}
impl From<LaserLines> for super::SpecDistType {
    fn from(laser_lines: LaserLines) -> Self {
        Self::LaserLines(laser_lines)
    }
}
