use std::{error::Error, fmt::Display};

#[derive(Debug, Clone)]
pub enum OpossumError {
    OpticScenery(String),
    OpticGroup(String),
    OpticPort(String),
    Analysis(String),
    Other(String),
}

impl Display for OpossumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpossumError::OpticScenery(m) => {
                write!(f, "Opossum Error::OpticScenery::{}", m)
            }
            OpossumError::OpticGroup(m) => {
                write!(f, "Opossum Error::OpticGroup::{}", m)
            }
            OpossumError::OpticPort(m) => {
                write!(f, "Opossum Error::OpticPort::{}", m)
            }
            OpossumError::Analysis(m) => {
                write!(f, "Opossum Error::Analysis::{}", m)
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