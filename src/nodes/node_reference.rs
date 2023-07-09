use std::cell::RefCell;
use std::rc::{Weak, Rc};

use crate::optic_node::{OpticNode, Optical, Dottable};
use crate::optic_ports::OpticPorts;

#[derive(Debug)]
/// A virtual component referring to another existing component. This node type is necessary in order to model resonators (loops) or double-pass systems.
pub struct NodeReference {
    reference: Weak<RefCell<OpticNode>>,
}

impl NodeReference {
    pub fn new(node: Rc<RefCell<OpticNode>>) -> OpticNode {
        let node_ref = Self { reference: Rc::downgrade(&node) };
        OpticNode::new(&format!("Ref: \"{}\"", &node.borrow().name()), node_ref)
    }
}

impl Optical for NodeReference {
    fn node_type(&self) -> &str {
        "reference"
    }

    fn ports(&self) -> OpticPorts {
       self.reference.upgrade().unwrap().borrow().ports().clone()
    }
}

impl Dottable for NodeReference{
    fn node_color(&self) -> &str {
        "lightsalmon3"
      }
}
