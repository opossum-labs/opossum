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
    pub fn analyze(&self) {
        for node in self.scene.nodes_topological().unwrap() {
            print!("Node: {}: ", node.name());
            node.analyze(AnalyzerType::Energy);
            println!("");
        }
    }
}

pub enum AnalyzerType {
    Energy
}