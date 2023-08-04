use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::analyzer::AnalyzerType;
use crate::error::OpossumError;
use crate::optic_node::{Dottable, LightResult, OpticNode, Optical};
use crate::optic_ports::OpticPorts;

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug)]
/// A virtual component referring to another existing component.
///
/// This node type is necessary in order to model resonators (loops) or double-pass systems.
///
/// ## Optical Ports
///   - Inputs
///     - input ports of the referenced [`OpticNode`]
///   - Outputs
///     - output ports of the referenced [`OpticNode`]
pub struct NodeReference {
    reference: Weak<RefCell<OpticNode>>,
}

impl NodeReference {
    /// Create new [`OpticNode`] (of type [`NodeReference`]) from another existing [`OpticNode`].
    pub fn from_node(node: Rc<RefCell<OpticNode>>) -> OpticNode {
        let node_ref = Self {
            reference: Rc::downgrade(&node),
        };
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

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        self.reference
            .upgrade()
            .unwrap()
            .borrow_mut()
            .analyze(incoming_data, analyzer_type)
    }
}

impl Dottable for NodeReference {
    fn node_color(&self) -> &str {
        "lightsalmon3"
    }
}
