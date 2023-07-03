use std::rc::{Weak, Rc};

use crate::optic_node::{OpticNode, Optical, Dottable};
use crate::optic_ports::OpticPorts;

/// A virtual component referring to another existing component. This node type is necessary in order to model resonators (loops) or double-pass systems.
pub struct NodeReference {
    reference: Weak<OpticNode>,
}

impl NodeReference {
    pub fn new(node: Rc<OpticNode>) -> OpticNode {
        let node_ref = Self { reference: Rc::downgrade(&node) };
        OpticNode::new(&format!("Ref: \"{}\"", &node.name()), node_ref)
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

impl Dottable for NodeReference{
    fn node_color(&self) -> &str {
        "lightsalmon3"
      }
}
