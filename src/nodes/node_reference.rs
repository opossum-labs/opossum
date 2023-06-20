use std::rc::{Weak, Rc};

use crate::optic_node::{OpticNode, Optical};

/// A virtual component referring to another existing component. This node type is necessary in order to model resonators (loops) or double-pass systems.
pub struct NodeReference {
    reference: Rc<OpticNode>,
}

impl NodeReference {
    pub fn new(node: Rc<OpticNode>) -> Self {
        Self { reference: node }
    }
}

impl Optical for NodeReference {
    fn node_type(&self) -> &str {
        "reference"
    }
}
