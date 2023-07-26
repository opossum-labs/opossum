use crate::{error::OpossumError, optic_scenery::OpticScenery};

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

pub enum AnalyzerType {
    Energy,
    ParAxialRayTrace,
}
