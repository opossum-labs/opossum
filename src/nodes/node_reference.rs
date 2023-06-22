use std::rc::{Weak, Rc};

use crate::optic_node::{OpticNode, Optical};
use crate::optic_ports::OpticPorts;

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
    fn to_dot(&self, node_index: &str, name: &str, inverted: bool) -> String {
        let inv_string = if inverted { "(inv)" } else { "" };
        let ref_name= self.reference.name();
        format!("  {} [label=\"Ref{} to {}\" shape=\"rect\"]\n", node_index, inv_string, ref_name)
    }
    fn ports(&self) -> OpticPorts {
       self.reference.ports().clone()
    }
}
