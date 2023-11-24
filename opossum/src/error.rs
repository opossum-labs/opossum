#![warn(missing_docs)]
//! Opossum specfic error structures
use std::{error::Error, fmt::Display};

/// Opossum application specific Result type
pub type OpmResult<T> = std::result::Result<T, OpossumError>;

/// Errors that can be returned by various OPOSSUM functions.
#[derive(Debug, PartialEq, Eq)]
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
            Self::OpticScenery(m) => {
                write!(f, "OpticScenery:{m}")
            }
            Self::OpticGroup(m) => {
                write!(f, "OpticGroup:{m}")
            }
            Self::OpticPort(m) => {
                write!(f, "OpticPort:{m}")
            }
            Self::Analysis(m) => {
                write!(f, "Analysis:{m}")
            }
            Self::Spectrum(m) => {
                write!(f, "Spectrum:{m}")
            }
            Self::Properties(m) => {
                write!(f, "Properties:{m}")
            }
            Self::Console(m) => {
                write!(f, "Console:{m}")
            }
            Self::Other(m) => write!(f, "Opossum Error:Other:{m}"),
        }
    }
}
impl Error for OpossumError {}

impl std::convert::From<String> for OpossumError {
    fn from(msg: String) -> Self {
        Self::Other(msg)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn from() {
        let error = OpossumError::from("test".to_string());
        assert_eq!(error, OpossumError::Other("test".to_string()));
    }
    #[test]
    fn display() {
        assert_eq!(
            format!("{}", OpossumError::OpticScenery("test".to_string())),
            "OpticScenery:test"
        );
        assert_eq!(
            format!("{}", OpossumError::OpticGroup("test".to_string())),
            "OpticGroup:test"
        );
        assert_eq!(
            format!("{}", OpossumError::OpticPort("test".to_string())),
            "OpticPort:test"
        );
        assert_eq!(
            format!("{}", OpossumError::Analysis("test".to_string())),
            "Analysis:test"
        );
        assert_eq!(
            format!("{}", OpossumError::Spectrum("test".to_string())),
            "Spectrum:test"
        );
        assert_eq!(
            format!("{}", OpossumError::Properties("test".to_string())),
            "Properties:test"
        );
        assert_eq!(
            format!("{}", OpossumError::Console("test".to_string())),
            "Console:test"
        );
        assert_eq!(
            format!("{}", OpossumError::Other("test".to_string())),
            "Opossum Error:Other:test"
        );
    }
    #[test]
    fn debug() {
        assert_eq!(
            format!("{:?}", OpossumError::OpticScenery("test".to_string())),
            "OpticScenery(\"test\")"
        );
    }
}
