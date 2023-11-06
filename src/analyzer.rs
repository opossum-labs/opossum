//! Optical Analyzers
use std::fmt::Display;

use crate::{error::OpmResult, optic_scenery::OpticScenery};
use strum::EnumIter;

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
    pub fn analyze(&mut self) -> OpmResult<()> {
        self.scene.analyze(&AnalyzerType::Energy)
    }
}

#[non_exhaustive]
#[derive(EnumIter, PartialEq, Debug)]
pub enum AnalyzerType {
    Energy,
    RayTrace,
}

impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            AnalyzerType::Energy => "energy",
            AnalyzerType::RayTrace => "ray tracing",
        };
        write!(f, "{}", msg)
    }
}
