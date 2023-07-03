use crate::optic_scenery::OpticScenery;

pub trait Analyzer {}

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
    pub fn analyze(&self) {
        for node in self.scene.nodes_topological().unwrap() {
            println!("Node: {}", node.name())
        }
    }
}

impl Analyzer for AnalyzerEnergy {}