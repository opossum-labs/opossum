//! Builder for the generation of energy spectra.
//!
//! This module provides a builder for the generation of energy spectra to be used in `LightData::Energy`.
//! Using this builder allows easier serialization / deserialization in OPM files.
use crate::{
    error::{OpmResult, OpossumError},
    joule, nanometer,
    spectrum::Spectrum,
    utils::default_from_name::DefaultFromName,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf};
use strum::EnumIter;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{Energy, Length},
        length::nanometer,
    },
};

use super::LightData;

/// Builder for the generation of energy spectra.
#[derive(Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum EnergyDataBuilder {
    /// Build a spectrum from raw data.
    Raw(Spectrum),
    /// Build a spectrum from a (CSV) file.
    FromFile(PathBuf),
    /// Build a spectrum from a set of (narrow) laser lines (center wavelength, energy) and a given spectrum resolution.
    LaserLines(EnergyLaserLines),
}
impl EnergyDataBuilder {
    /// Build the spectrum from the builder.
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the builder fails.
    pub fn build(&self) -> OpmResult<LightData> {
        match self {
            Self::Raw(s) => Ok(LightData::Energy(s.clone())),
            Self::FromFile(p) => {
                let spectrum = Spectrum::from_csv(p)?;
                Ok(LightData::Energy(spectrum))
            }
            Self::LaserLines(e) => {
                let spectrum =
                    Spectrum::from_laser_lines(e.lines().clone(), *e.spectral_resolution())?;
                Ok(LightData::Energy(spectrum))
            }
        }
    }
}

impl DefaultFromName for EnergyDataBuilder {}

impl Default for EnergyDataBuilder {
    fn default() -> Self {
        Self::LaserLines(EnergyLaserLines::default())
    }
}

impl std::fmt::Debug for EnergyDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(s) => write!(f, "Raw({s:?})"),
            Self::FromFile(p) => write!(f, "FromFile({:?})", p.display()),
            Self::LaserLines(e) => {
                write!(
                    f,
                    "LaserLines({:?}, {:.3})",
                    e.lines,
                    e.spectral_resolution()
                        .into_format_args(nanometer, Abbreviation)
                )
            }
        }
    }
}

impl Display for EnergyDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(_) => write!(f, "Raw"),
            Self::FromFile(_) => write!(f, "From File"),
            Self::LaserLines(_) => write!(f, "LaserLines"),
        }
    }
}

impl From<EnergyLaserLines> for EnergyDataBuilder {
    fn from(value: EnergyLaserLines) -> Self {
        Self::LaserLines(value)
    }
}

impl From<PathBuf> for EnergyDataBuilder {
    fn from(value: PathBuf) -> Self {
        Self::FromFile(value)
    }
}

/// A struct that contains laser line information for the [`EnergyDataBuilder`] variant `LaserLines`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnergyLaserLines {
    lines: Vec<(Length, Energy)>,
    spectral_resolution: Length,
}

impl EnergyLaserLines {
    /// Creates a new `EnergyLaserLines` instance with the given laser lines.
    ///
    /// # Arguments
    ///
    /// * `lines` - A vector of tuples containing the wavelength and energy of each laser line.
    /// * `spectral_resolution` - the spectral width of each line
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// * the vector is empty,
    /// * any wavelength is negative or infinite,
    /// * any energy is zero, negative or infinite,
    pub fn new(lines: Vec<(Length, Energy)>, spectral_resolution: Length) -> OpmResult<Self> {
        // Check if the lines are non-empty and contain valid data
        if lines.is_empty() {
            return Err(OpossumError::Other("Laser lines cannot be empty".into()));
        }

        if !spectral_resolution.is_normal() {
            return Err(OpossumError::Other(
                "Spectral resolution must be positive and finite".into(),
            ));
        }
        for (wavelength, energy) in &lines {
            if !wavelength.is_normal() || wavelength.is_sign_negative() {
                return Err(OpossumError::Other(
                    "Wavelength must be positive and finite".into(),
                ));
            }
            if !energy.is_normal() || energy.is_sign_negative() {
                return Err(OpossumError::Other(
                    "Energy must be positive and finite".into(),
                ));
            }
        }

        Ok(Self {
            lines,
            spectral_resolution,
        })
    }

    /// Creates a new, empty [`EnergyLaserLines`] distribution.
    ///
    /// This initializes the internal storage without any spectral lines and a spectral resolution of 0.1 nm.
    ///
    /// # Returns
    /// A new instance of [`EnergyLaserLines`] with an empty set of wavelength–intensity pairs.
    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            lines: Vec::<(Length, Energy)>::new(),
            spectral_resolution: nanometer!(0.1),
        }
    }

    /// Adds a list of laser lines to the [`EnergyLaserLines`] distribution.
    ///
    /// Each laser line is a tuple containing a [`Length`] representing the wavelength,
    /// and an `Energy`.
    ///
    /// # Parameters
    /// * `lines` – A vector of `(Length, Energy)` tuples, each representing a spectral line.
    ///
    /// # Returns
    /// * `Ok(())` if all lines are valid and added successfully.
    /// * `Err(OpossumError)` if validation fails.
    ///
    /// # Errors
    /// This method returns an error if:
    /// - The input list is empty.
    /// - Any wavelength is negative or not finite.
    /// - Any Energy is negative or not finite.
    pub fn add_lines(&mut self, lines: Vec<(Length, Energy)>) -> OpmResult<()> {
        // Check if the lines are non-empty and contain valid data
        if lines.is_empty() {
            return Err(OpossumError::Other("Laser lines cannot be empty".into()));
        }
        for (wavelength, energy) in &lines {
            if !wavelength.is_normal() || wavelength.is_sign_negative() {
                return Err(OpossumError::Other(
                    "Wavelength must be positive and finite".into(),
                ));
            }
            if !energy.is_normal() || energy.is_sign_negative() {
                return Err(OpossumError::Other(
                    "Energy must be positive and finite".into(),
                ));
            }
        }
        for line in lines {
            self.lines.push(line);
        }
        Ok(())
    }

    /// Returns an immutable reference to the list of laser lines stored in this [`EnergyLaserLines`] instance.
    ///
    /// Each line is represented as a tuple `(Length, Energy)`, where the `Length` is the wavelength and
    /// `Energy` is the corresponding energy of the line.
    ///
    /// # Returns
    /// A reference to the vector of spectral lines.
    #[must_use]
    pub fn lines(&self) -> &Vec<(Length, Energy)> {
        &self.lines
    }

    /// Returns an immutable reference to the `spectral_resolution` stored in this [`EnergyLaserLines`] instance.
    ///
    /// # Returns
    /// A reference to the `spectral_resolution` of thes spectral lines.
    #[must_use]
    pub fn spectral_resolution(&self) -> &Length {
        &self.spectral_resolution
    }

    /// Sets the `spectral_resolution`
    pub fn set_spectral_resolution(&mut self, spectral_resolution: Length) {
        self.spectral_resolution = spectral_resolution;
    }

    /// removes a laser line from [`EnergyLaserLines`] at a specific index
    pub fn delete_line(&mut self, index: usize) {
        if index < self.lines.len() {
            self.lines.remove(index);
        }
    }
}

impl Default for EnergyLaserLines {
    fn default() -> Self {
        Self {
            lines: vec![(nanometer!(1054.), joule!(1.))],
            spectral_resolution: nanometer!(0.1),
        }
    }
}
