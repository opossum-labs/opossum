use std::rc::{Weak, Rc};

use crate::optic_node::{OpticNode, Optical};
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
    fn to_dot(&self, node_index: &str, _name: &str, inverted: bool) -> String {
        let inv_string = if inverted { "(inv)" } else { "" };
        let node_ref= self.reference.upgrade().unwrap();
        format!("  {} [label=\"Ref{} to {}\" shape=\"rect\"]\n", node_index, inv_string, node_ref.name())
    }
    fn ports(&self) -> OpticPorts {
       self.reference.upgrade().unwrap().ports().clone()
    }
}
