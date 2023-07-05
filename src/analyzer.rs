use crate::optic_scenery::OpticScenery;

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
    pub fn analyze(&mut self) {
       self.scene.analyze(&AnalyzerType::Energy).unwrap();
    }
}

pub enum AnalyzerType {
    Energy
}