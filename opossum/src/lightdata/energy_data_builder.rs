//! Builder for the generation of energy spectra.
//!
//! This module provides a builder for the generation of energy spectra to be used in `LightData::Energy`.
//! Using this builder allows easier serialization / deserialization in OPM files.
use crate::{error::OpmResult, spectrum::Spectrum};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{Energy, Length},
        length::nanometer,
    },
};

use super::LightData;

/// Builder for the generation of energy spectra.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EnergyDataBuilder {
    /// Build a spectrum from raw data.
    Raw(Spectrum),
    /// Build a spectrum from a (CSV) file.
    FromFile(PathBuf),
    /// Build a spectrum from a set of (narrow) laser lines (center wavelength, energy) and a given spectrum resolution.
    LaserLines(Vec<(Length, Energy)>, Length),
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
            Self::LaserLines(l, r) => {
                let spectrum = Spectrum::from_laser_lines(l.clone(), *r)?;
                Ok(LightData::Energy(spectrum))
            }
        }
    }
}

impl Display for EnergyDataBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw(s) => write!(f, "Raw({s})"),
            Self::FromFile(p) => write!(f, "FromFile({})", p.display()),
            Self::LaserLines(l, r) => {
                write!(
                    f,
                    "LaserLines({:?}, {:.3})",
                    l,
                    r.into_format_args(nanometer, Abbreviation)
                )
            }
        }
    }
}
