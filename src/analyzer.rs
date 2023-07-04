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
        for node_edges in self.scene.nodes_topological().unwrap() {
            print!("Node: {}: ", node_edges.0.name());
            node_edges.0.analyze(node_edges.1,AnalyzerType::Energy);
            println!("");
        }
    }
}

pub enum AnalyzerType {
    Energy
}