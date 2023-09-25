use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpossumError;
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, Optical};
use crate::properties::Properties;

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Debug, Default)]
/// A virtual component referring to another existing component.
///
/// This node type is necessary in order to model resonators (loops) or double-pass systems.
///
/// ## Optical Ports
///   - Inputs
///     - input ports of the referenced [`Optical`]
///   - Outputs
///     - output ports of the referenced [`Optical`]
pub struct NodeReference {
    reference: Option<Weak<RefCell<dyn Optical>>>,
    props: Properties
}

impl NodeReference {
    // Create new [`OpticNode`] (of type [`NodeReference`]) from another existing [`OpticNode`].
    pub fn from_node(node: Rc<RefCell<dyn Optical>>) -> Self {
        Self {
            reference: Some(Rc::downgrade(&node)),
            props: Properties::default()
        }
    }
}

impl Optical for NodeReference {
    fn node_type(&self) -> &str {
        "reference"
    }

    fn ports(&self) -> OpticPorts {
        if let Some(rf)= &self.reference {
            rf.upgrade().unwrap().borrow().ports().clone()
        } else {
            OpticPorts::default()
        }
        
    }

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        if let Some(rf)= &self.reference {
            rf
            .upgrade()
            .unwrap()
            .borrow_mut()
            .analyze(incoming_data, analyzer_type)
        } else {
            Err(OpossumError::Analysis("reference node has no reference defined".into()))
        }
       
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
}

impl Dottable for NodeReference {
    fn node_color(&self) -> &str {
        "lightsalmon3"
    }
}
