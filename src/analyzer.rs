//! Optical Analyzers
use std::fmt::Display;

use crate::{error::OpossumError, optic_scenery::OpticScenery};
use strum::EnumIter;

type Result<T> = std::result::Result<T, OpossumError>;
#[derive(Debug)]
pub struct AnalyzerEnergy {
    scene: OpticScenery,
}

impl AnalyzerEnergy {
    pub fn new(scenery: &OpticScenery) -> Self {
        Self {
            scene: (*scenery).to_owned(),
        }
    }
    pub fn analyze(&mut self) -> Result<()> {
        self.scene.analyze(&AnalyzerType::Energy)
    }
}

#[non_exhaustive]
#[derive(EnumIter, PartialEq, Debug)]
pub enum AnalyzerType {
    Energy,
    ParAxialRayTrace,
}

impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            AnalyzerType::Energy => "energy",
            AnalyzerType::ParAxialRayTrace => "paraxial ray tracing",
        };
        write!(f, "{}", msg)
    }
}
