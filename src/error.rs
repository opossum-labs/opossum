#![warn(missing_docs)]
//! Opossum specfic error structures
use std::{error::Error, fmt::Display};

/// Opossum application specific Result type
pub type OpmResult<T> = std::result::Result<T, OpossumError>;

/// Errors that can be returned by various OPOSSUM functions.
#[derive(Debug, Clone)]
pub enum OpossumError {
    /// error while setting up an `OpticScenery`
    OpticScenery(String),
    /// error while setting up an `OpticGroup`. The reasons are similar to [`OpossumError::OpticScenery`]
    OpticGroup(String),
    /// (mostly internal) errors while dealing with optical ports.
    OpticPort(String),
    /// mostly runtime errors occuring during the analysis of a scenery
    Analysis(String),
    /// errors while handling optical spectra
    Spectrum(String),
    /// errors console io
    Console(String),
    /// errors in connection with properties handling
    Properties(String),
    /// errors not falling in one of the categories above
    Other(String),
}

impl Display for OpossumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpossumError::OpticScenery(m) => {
                write!(f, "OpticScenery::{}", m)
            }
            OpossumError::OpticGroup(m) => {
                write!(f, "OpticGroup::{}", m)
            }
            OpossumError::OpticPort(m) => {
                write!(f, "OpticPort::{}", m)
            }
            OpossumError::Analysis(m) => {
                write!(f, "Analysis::{}", m)
            }
            OpossumError::Spectrum(m) => {
                write!(f, "Spectrum::{}", m)
            }
            OpossumError::Properties(m) => {
                write!(f, "Properties::{}", m)
            }
            OpossumError::Console(m) => {
                write!(f, "Console::{}", m)
            }
            OpossumError::Other(m) => write!(f, "Opossum Error::Other::{}", m),
        }
    }
}
impl Error for OpossumError {}

impl std::convert::From<String> for OpossumError {
    fn from(msg: String) -> Self {
        Self::Other(msg)
    }
}
