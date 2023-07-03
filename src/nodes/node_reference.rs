use std::rc::{Weak, Rc};

use crate::optic_node::{OpticNode, Optical, DotScenery};
use crate::optic_ports::OpticPorts;

/// A virtual component referring to another existing component. This node type is necessary in order to model resonators (loops) or double-pass systems.
pub struct NodeReference {
    reference: Weak<OpticNode>,
}

impl NodeReference {
    pub fn new(node: Rc<OpticNode>) -> Self {
        Self { reference: Rc::downgrade(&node) }
    }
}

impl Optical for NodeReference {
    fn node_type(&self) -> &str {
        "reference"
    }

    fn ports(&self) -> OpticPorts {
       self.reference.upgrade().unwrap().ports().clone()
    }
}

impl DotScenery for NodeReference{}
