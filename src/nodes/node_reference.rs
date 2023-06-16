use std::rc::{Weak, Rc};

use crate::optic_node::{OpticNode, Optical};

/// A virtual component referring to another existing component. This node type is necessary in order to model resonators (loops) or double-pass systems.
pub struct NodeReference {
    reference: Rc<dyn Optical>,
}

impl NodeReference {
    pub fn new(node: &OpticNode) -> Self {
        Self { reference: node.node_ref() }
    }
}

impl Optical for NodeReference {
    fn node_type(&self) -> &str {
        "reference"
    }
}
