use std::rc::{Weak, Rc};

use crate::optic_node::{OpticNode, Optical};

/// A virtual component referring to another existing component. This node type is necessary in order to model resonators (loops) or double-pass systems.
pub struct NodeReference {
    reference: Weak<OpticNode>,
}

impl NodeReference {
    pub fn new(node: OpticNode) -> Self {
        Self { reference: Rc::downgrade(&Rc::new(node)) }
    }
}

impl Optical for NodeReference {
    fn node_type(&self) -> &str {
        "reference"
    }
}
