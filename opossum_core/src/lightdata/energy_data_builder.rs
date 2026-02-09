//! Builder for the generation of energy spectra.
//!
//! This module provides a builder for the generation of energy spectra to be used in `LightData::Energy`.
//! Using this builder allows easier serialization / deserialization in OPM files.
use crate::{
    error::{OpmResult, OpossumError},
    generic_validators::{
        AllNormal, AllNotEmpty, AllPositive, PathValid, ValidateTrait, XNormal, YFinite,
        YNotAllZero,
    },
    joule, nanometer,
    spectral_distribution::laser_lines::MIN_WAVELENGTH_DIFF_NM,
    spectrum::Spectrum,
    utils::default_from_name::DefaultFromName,
    validated, validated_type, validated_vec, validated_vec_type,
};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf};
use strum::EnumIter;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        energy::joule,
        f64::{Energy, Length},
        length::nanometer,
    },
};

use super::LightData;

/// Builder for the generation of energy spectra.
#[derive(Clone, Serialize, Deserialize, PartialEq, EnumIter, EnsureValidated)]
pub enum EnergyDataBuilder {
    /// Build a spectrum from raw data.
    Raw(Spectrum),
    /// Build a spectrum from a (CSV) file.
    FromFile(SpectrumFile),
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
                let spectrum = Spectrum::from_csv(p.f_path())?;
                Ok(LightData::Energy(spectrum))
            }
            Self::LaserLines(e) => {
                let spectrum = Spectrum::from_laser_lines(e)?;
                Ok(LightData::Energy(spectrum))
            }
        }
    }
}

#[derive(Deserialize)]
struct NonValidatedSpectrumFile {
    pub f_path: PathBuf,
}

/// Struct to store a path to read a spectrum from file
#[derive(Clone, Serialize, PartialEq, EnsureValidated, Debug, Eq)]
pub struct SpectrumFile {
    f_path: validated_type!(PathBuf, PathValid),
}

impl<'de> serde::Deserialize<'de> for SpectrumFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        //deserialize non validated struct
        let helper = NonValidatedSpectrumFile::deserialize(deserializer)?;

        //get correct validators from default
        Self::new(helper.f_path).map_err(serde::de::Error::custom)
    }
}

impl SpectrumFile {
    ///Create a new [`SpectrumFile`]
    ///
    /// # Errors
    /// Returns an error if path validation fails
    pub fn new(f_path: PathBuf) -> OpmResult<Self> {
        let mut spec_file = Self::default();
        spec_file.set_f_path(f_path)?;
        Ok(spec_file)
    }
    /// Return the path to a spectrum file if defined
    #[must_use]
    pub const fn f_path(&self) -> &PathBuf {
        self.f_path.get()
    }
    /// set the path to spectrum file
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_f_path(&mut self, f_path: PathBuf) -> OpmResult<()> {
        self.f_path.set(f_path)?;
        Ok(())
    }
}

impl Default for SpectrumFile {
    fn default() -> Self {
        Self {
            f_path: validated!(
                PathBuf::from("empty.csv"),
                PathValid::new(Some(vec!["csv"]))
            )
            .unwrap(),
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
            Self::FromFile(p) => write!(f, "FromFile({})", p.f_path().display()),
            Self::LaserLines(e) => {
                write!(f, "LaserLines([").unwrap();
                for (wvl, energy) in &e.lines() {
                    write!(
                        f,
                        "({:.3}, {:.3})",
                        wvl.into_format_args(nanometer, Abbreviation),
                        energy.into_format_args(joule, Abbreviation)
                    )
                    .unwrap();
                }
                write!(
                    f,
                    "] resolution: {:.3})",
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

impl From<SpectrumFile> for EnergyDataBuilder {
    fn from(value: SpectrumFile) -> Self {
        Self::FromFile(value)
    }
}

/// A struct that contains laser line information for the [`EnergyDataBuilder`] variant `LaserLines`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnsureValidated)]
pub struct EnergyLaserLines {
    lines: validated_vec_type!(
        Vec<(Length, Energy)>,
        AllPositive && XNormal && YFinite,
        AllNotEmpty && YNotAllZero
    ),
    spectral_resolution: validated_type!(Length, AllNormal && AllPositive),
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
        let mut laser_lines = Self::default();
        laser_lines.set_lines(lines)?;
        laser_lines.set_spectral_resolution(spectral_resolution)?;
        Ok(laser_lines)
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
    /// - Any wavelength is negative or not normal.
    /// - Any Energy is negative or not finite.
    pub fn add_lines(&mut self, lines: Vec<(Length, Energy)>) -> OpmResult<()> {
        let current_lines = self.lines.get();
        for (new_wvl, _) in &lines {
            // Check against existing lines
            if current_lines.iter().any(|(current_wvl, _)| {
                (*current_wvl - *new_wvl).abs() < Length::new::<nanometer>(MIN_WAVELENGTH_DIFF_NM)
            }) {
                return Err(crate::error::OpossumError::Spectrum(format!(
                    "Laser line with wavelength {:.6} nm already exists",
                    new_wvl.get::<uom::si::length::nanometer>()
                )));
            }
        }

        // Check for duplicates within the new lines
        for (i, (wvl1, _)) in lines.iter().enumerate() {
            for (wvl2, _) in lines.iter().skip(i + 1) {
                if (*wvl1 - *wvl2).abs() < Length::new::<nanometer>(MIN_WAVELENGTH_DIFF_NM) {
                    return Err(crate::error::OpossumError::Spectrum(format!(
                        "Duplicate laser line with wavelength {:.6} nm in input",
                        wvl1.get::<uom::si::length::nanometer>()
                    )));
                }
            }
        }

        for line in lines {
            self.lines.push(line)?;
        }
        Ok(())
    }

    /// Sets a list of energy laser lines to the [`EnergyLaserLines`] distribution.
    ///
    /// Each laser line is a tuple containing a [`Length`] representing the wavelength,
    /// and a [`Energy`] representing the energy.
    ///
    /// # Parameters
    /// * `lines` – A vector of `(Length, Energy)` tuples, each representing an energy laser line.
    ///
    /// # Returns
    /// * `Ok(())` if all lines are valid and added successfully.
    /// * `Err(OpossumError)` if validation fails.
    ///
    /// # Errors
    /// This method returns an error if:
    /// - The input list is empty.
    /// - Any wavelength is negative or not normal.
    /// - Any energy is negative or not finite.
    pub fn set_lines(&mut self, lines: Vec<(Length, Energy)>) -> OpmResult<()> {
        // Check for duplicates within the new lines
        for (i, (wvl1, _)) in lines.iter().enumerate() {
            for (wvl2, _) in lines.iter().skip(i + 1) {
                if (*wvl1 - *wvl2).abs() < Length::new::<nanometer>(MIN_WAVELENGTH_DIFF_NM) {
                    return Err(crate::error::OpossumError::Spectrum(format!(
                        "Duplicate laser line with wavelength {:.6} nm in input",
                        wvl1.get::<uom::si::length::nanometer>()
                    )));
                }
            }
        }
        self.lines.set(lines)?;
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
    pub fn lines(&self) -> Vec<(Length, Energy)> {
        self.lines
            .get()
            .iter()
            .map(|l| (l.0, l.1))
            .collect::<Vec<(Length, Energy)>>()
    }

    /// Returns an immutable reference to the `spectral_resolution` stored in this [`EnergyLaserLines`] instance.
    ///
    /// # Returns
    /// A reference to the `spectral_resolution` of thes spectral lines.
    #[must_use]
    pub fn spectral_resolution(&self) -> &Length {
        self.spectral_resolution.get()
    }

    /// Sets the `spectral_resolution`
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_spectral_resolution(&mut self, spectral_resolution: Length) -> OpmResult<()> {
        self.spectral_resolution.set(spectral_resolution)?;
        Ok(())
    }

    /// removes a laser line from [`EnergyLaserLines`] at a specific index
    ///
    /// # Errors
    /// Returns an error if
    /// - validation fails
    /// - given line index is out of bounds
    pub fn delete_line(&mut self, index: usize) -> OpmResult<()> {
        let lines = self.lines.get();
        if index < lines.len() {
            let mut lines = lines.clone();
            lines.remove(index);
            self.lines.set(lines)?;
            return Ok(());
        }
        Err(OpossumError::Other("line index out of bounds".into()))
    }
}

impl Default for EnergyLaserLines {
    fn default() -> Self {
        Self {
            lines: validated_vec!(
                vec![(nanometer!(1054.), joule!(1.))],
                AllPositive && XNormal && YFinite,
                AllNotEmpty && YNotAllZero
            )
            .unwrap(),
            spectral_resolution: validated!(nanometer!(0.1), AllNormal && AllPositive).unwrap(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn spectrum_file_new() {
        assert!(
            SpectrumFile::new("path.csv".into()).is_ok(),
            // "SpectrumFile must have the suffix *.csv"
        );
        assert!(
            SpectrumFile::new("path.txt".into()).is_err(),
            // "SpectrumFile must have the suffix *.csv"
        );
    }
    #[test]
    fn spectrum_file_default() {
        let s = SpectrumFile::default();
        assert_eq!(s.f_path(), "empty.csv");
    }
    #[test]
    fn spectrum_file_file_path() {
        let s = SpectrumFile::new("path.csv".into()).unwrap();
        assert_eq!(s.f_path(), "path.csv");
    }
    #[test]
    fn spectrum_file_deserialize() {
        let s = SpectrumFile::default();
        let serialized =
            ron::ser::to_string_pretty(&s, ron::ser::PrettyConfig::new().new_line("\n")).unwrap();
        assert!(ron::from_str::<SpectrumFile>(&serialized).is_ok());
    }
    #[test]
    fn energy_laser_lines_new() {
        assert!(
            EnergyLaserLines::new(vec![(nanometer!(500.0), joule!(0.1))], nanometer!(1.0)).is_ok(),
            // "reasonable values should be OK"
        );
        assert!(
            EnergyLaserLines::new(vec![(nanometer!(500.0), joule!(0.1))], nanometer!(0.0)).is_err(),
            // "resolution <=0.0 nm is an error"
        );
        assert!(
            EnergyLaserLines::new(vec![(nanometer!(500.0), joule!(-0.1))], nanometer!(1.0))
                .is_err(),
            // "negative line energy is an error"
        );
        assert!(
            EnergyLaserLines::new(vec![(nanometer!(0.0), joule!(0.1))], nanometer!(1.0)).is_err(),
            // "zero wavelength line is an error"
        );
        assert!(
            EnergyLaserLines::new(
                vec![
                    (nanometer!(500.0), joule!(0.1)),
                    (nanometer!(510.0), joule!(0.0))
                ],
                nanometer!(1.0)
            )
            .is_ok(),
            // "at least one laser line must have a non-zero energy"
        );
        assert!(
            EnergyLaserLines::new(vec![], nanometer!(1.0)).is_err(),
            // "empty laser lines is an error"
        );
    }
    #[test]
    fn energy_laser_lines_default() {
        let ell = EnergyLaserLines::default();
        assert_eq!(ell.lines(), vec![(nanometer!(1054.0), joule!(1.0))]);
        assert_eq!(ell.spectral_resolution(), &nanometer!(0.1));
    }
    #[test]
    fn energy_laser_lines_add_lines() {
        let mut ell = EnergyLaserLines::default();
        assert!(
            ell.add_lines(vec![(nanometer!(500.0), joule!(0.1))])
                .is_ok()
        );
        assert_eq!(
            ell.lines(),
            vec![
                (nanometer!(1054.0), joule!(1.0)),
                (nanometer!(500.0), joule!(0.1))
            ]
        );
        assert!(
            ell.add_lines(vec![(nanometer!(500.0), joule!(-0.1))])
                .is_err(),
            // "a line with negative energy is an error"
        );
        assert!(
            ell.add_lines(vec![(nanometer!(0.0), joule!(0.1))]).is_err(),
            // "a zero-wavelength line is an error"
        );
        assert!(
            ell.add_lines(vec![]).is_ok(),
            // "It OK to add an empty line entry (but does it make sense?)"
        );
        assert!(
            ell.add_lines(vec![(nanometer!(500.0), joule!(0.1))])
                .is_err() // duplicate line wvl: 500nm
        );
        assert!(
            ell.add_lines(vec![
                (nanometer!(600.0), joule!(0.1)),
                (nanometer!(600.0), joule!(0.1))
            ])
            .is_err() // duplicate line wvl in input: 600nm
        )
    }
    #[test]
    fn energy_laser_lines_set_lines() {
        let mut ell = EnergyLaserLines::default();
        assert!(
            ell.set_lines(vec![
                (nanometer!(500.0), joule!(0.5)),
                (nanometer!(505.0), joule!(1.5))
            ])
            .is_ok()
        );
        assert_eq!(
            ell.lines(),
            vec![
                (nanometer!(500.0), joule!(0.5)),
                (nanometer!(505.0), joule!(1.5))
            ]
        );
        assert!(
            ell.set_lines(vec![]).is_err(),
            // "Setting an empty array is an error"
        );
        assert!(
            ell.set_lines(vec![(nanometer!(0.0), joule!(0.5))]).is_err(),
            // "zero wavelength line is an error"
        );
        assert!(
            ell.set_lines(vec![(nanometer!(500.0), joule!(-0.5))])
                .is_err(),
            // "negative energy line is an error"
        );
        assert!(
            ell.set_lines(vec![
                (nanometer!(500.0), joule!(0.5)),
                (nanometer!(500.0), joule!(0.5))
            ])
            .is_err(),
            // "duplicate wavelength in input is an error"
        );
    }
    #[test]
    fn energy_laser_lines_delete_line() {
        let mut ell = EnergyLaserLines::default();
        ell.set_lines(vec![
            (nanometer!(500.0), joule!(0.5)),
            (nanometer!(505.0), joule!(1.5)),
        ])
        .unwrap();
        assert!(
            ell.delete_line(2).is_err(),
            // "deleting out of bounds index is an error"
        );
        assert!(ell.delete_line(0).is_ok());
        assert_eq!(ell.lines(), vec![(nanometer!(505.0), joule!(1.5))]);
        assert!(
            ell.delete_line(0).is_err(),
            // "deleting the last remaining line is an error"
        );
        assert_eq!(ell.lines(), vec![(nanometer!(505.0), joule!(1.5))]);
    }
    #[test]
    fn energy_data_builder_from_energy_laser_lines() {
        let ell = EnergyLaserLines::default();
        let edb: EnergyDataBuilder = ell.into();
        assert!(matches!(edb, EnergyDataBuilder::LaserLines(_)));
    }
    #[test]
    fn energy_data_builder_from_file() {
        let sf = SpectrumFile::default();
        let edb: EnergyDataBuilder = sf.into();
        assert!(matches!(edb, EnergyDataBuilder::FromFile(_)));
    }
    #[test]
    fn energy_data_builder_from_raw() {
        let s = Spectrum::new(nanometer!(500.0)..nanometer!(550.0), nanometer!(1.0)).unwrap();
        let edb: EnergyDataBuilder = s.into();
        assert!(matches!(edb, EnergyDataBuilder::Raw(_)));
    }
    #[test]
    fn energy_data_builder_build_display() {
        let ell = EnergyLaserLines::default();
        let edb: EnergyDataBuilder = ell.into();
        assert!(edb.build().is_ok());
        assert_eq!(format!("{edb}"), "LaserLines");
        assert_eq!(
            format!("{edb:?}"),
            "LaserLines([(1054.000 nm, 1.000 J)] resolution: 0.100 nm)"
        );
        let sf = SpectrumFile::new("./files_for_testing/spectrum/spec_to_csv_test_01.csv".into())
            .unwrap();
        let edb: EnergyDataBuilder = sf.into();
        assert!(edb.build().is_ok());
        assert_eq!(format!("{edb}"), "From File");
        assert_eq!(
            format!("{edb:?}"),
            "FromFile(./files_for_testing/spectrum/spec_to_csv_test_01.csv)"
        );
        let s = Spectrum::new(nanometer!(500.0)..nanometer!(503.0), nanometer!(1.0)).unwrap();
        let edb: EnergyDataBuilder = s.into();
        assert!(edb.build().is_ok());
        assert_eq!(format!("{edb}"), "Raw");
        assert_eq!(
            format!("{edb:?}"),
            "Raw( 500.00 nm -> 0\n 501.00 nm -> 0\n 502.00 nm -> 0\n)"
        );
    }
}
