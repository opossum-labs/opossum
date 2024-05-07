use crate::{nodes::ray_propagation_visualizer::RayPositionHistories, reporter::AnalysisReport};
use bevy::{ecs::system::Resource, render::mesh::Mesh};

#[derive(Resource, Clone)]
pub struct SceneryBevyData {
    ray_history: Option<RayPositionHistories>,
    node_meshes: Vec<Mesh>,
}

impl SceneryBevyData {
    pub fn from_report(report: &AnalysisReport) -> Self {
        let mut meshes: Vec<Mesh> = Vec::default();
        if let Some(scenery) = report.scenery() {
            for node in scenery.nodes() {
                meshes.push(node.optical_ref.borrow().mesh());
            }
        }
        Self {
            ray_history: report.get_ray_hist().cloned(),
            node_meshes: meshes,
        }
    }

    pub fn ray_history(&self) -> Option<&RayPositionHistories> {
        self.ray_history.as_ref()
    }

    pub fn node_meshes(&self) -> &[Mesh] {
        &self.node_meshes
    }
}
