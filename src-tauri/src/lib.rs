use opossum::{error::{OpmResult}, nodes::{create_node_ref, NodeGroup}};
use uuid::Uuid;
pub mod commands;

#[derive(Clone)]
pub struct OPMGUIModel {
    model: NodeGroup,
}

impl OPMGUIModel {
    pub fn new(name: &str) -> Self {
        Self { model: NodeGroup::new(name) }
    }

    pub fn add_default_node(&mut self, node_type: &str) -> OpmResult<Uuid> {
        let node = create_node_ref(node_type).unwrap();
        self.model.add_node_ref(&node)?;
        Ok(node.uuid())
    }
}

