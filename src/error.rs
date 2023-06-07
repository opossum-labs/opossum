use std::{error::Error, fmt::Display};

#[derive(Debug, Clone)]
pub enum OpossumError {
    OpticScenery(String),
    OpticGroup(String),
    OpticPort(String),
    Other(String),
}

impl Display for OpossumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpossumError::OpticScenery(m) => {
                f.write_fmt(format_args!("Opossum Error::OpticScenery::{}", m))
            }
            OpossumError::OpticGroup(m) => {
                f.write_fmt(format_args!("Opossum Error::OpticGroup::{}", m))
            }
            OpossumError::OpticPort(m) => {
                f.write_fmt(format_args!("Opossum Error::OpticPort::{}", m))
            }
            OpossumError::Other(m) => f.write_fmt(format_args!("Opossum Error::Other::{}", m)),
        }
    }
}
impl Error for OpossumError {}
