use opossum::{
    error::OpmResult,
    nodes::{create_node_ref, NodeGroup},
};
use uuid::Uuid;
pub mod commands;

#[derive(Clone)]
pub struct OPMGUIModel {
    model: NodeGroup,
}

unsafe impl Send for OPMGUIModel {}

impl OPMGUIModel {
    pub fn new(name: &str) -> Self {
        Self {
            model: NodeGroup::new(name),
        }
    }
    pub fn set_model(&mut self, model: NodeGroup) {
        self.model = model;
    }
    pub fn add_default_node(&mut self, node_type: &str) -> OpmResult<Uuid> {
        let node = create_node_ref(node_type).unwrap();
        self.model.add_node_ref(&node)?;
        Ok(node.uuid())
    }

    pub fn model(&self) -> &NodeGroup {
        &self.model
    }

    pub fn model_mut(&mut self) -> &mut NodeGroup {
        &mut self.model
    }
}
