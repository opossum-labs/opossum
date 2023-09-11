use serde::ser::SerializeStruct;
use serde::Serialize;

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::OpossumError;
use crate::lightdata::LightData;
use crate::nodes::NodeGroup;
use crate::optic_ports::OpticPorts;
use core::fmt::Debug;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
pub type LightResult = HashMap<String, Option<LightData>>;
type Result<T> = std::result::Result<T, OpossumError>;

/// This is the basic trait that must be implemented by all concrete optical components.
pub trait Optical: Dottable + erased_serde::Serialize {
    /// Sets the name of this [`Optical`].
    fn set_name(&mut self, _name: &str) {}
    /// Returns a reference to the name of this [`Optical`].
    fn name(&self) -> &str {
        self.node_type()
    }
    /// Return the type of the optical component (lens, filter, ...). The default implementation returns "undefined".
    fn node_type(&self) -> &str {
        "undefined"
    }
    /// Return the available (input & output) ports of this [`Optical`].
    fn ports(&self) -> OpticPorts {
        OpticPorts::default()
    }
    /// Perform an analysis of this element. The type of analysis is given by an [`AnalyzerType`].
    ///
    /// This function is normally only called by [`OpticScenery::analyze()`](crate::optic_scenery::OpticScenery::analyze()).
    ///
    /// # Errors
    ///
    /// This function will return an error if internal element-specific errors occur and the analysis cannot be performed.
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        _analyzer_type: &AnalyzerType,
    ) -> Result<LightResult> {
        print!("{}: No analyze function defined.", self.node_type());
        Ok(LightResult::default())
    }
    /// Export analysis data to file with the given name.
    fn export_data(&self, _file_name: &str) {
        println!(
            "no export_data function implemented for nodetype <{}>",
            self.node_type()
        )
    }
    /// Returns `true` if the [`Optical`] represents a detector which can report analysis data.
    fn is_detector(&self) -> bool {
        false
    }
    /// Mark this [`Optical`] as inverted.
    fn set_inverted(&mut self, _inverted: bool) {
        // self.ports.set_inverted(inverted);
        // self.node.set_inverted(inverted);
    }
    /// Returns `true` if this [`Optical`] is inverted.
    fn inverted(&self) -> bool {
        false
    }
    fn as_group(&self) -> Result<&NodeGroup> {
        Err(OpossumError::Other("cannot cast to group".into()))
    }
}

impl Debug for dyn Optical {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name(), self.node_type())
    }
}

#[derive(Debug, Clone)]
pub struct OpticRef(pub Rc<RefCell<dyn Optical>>);

impl Serialize for OpticRef {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut node = serializer.serialize_struct("node", 1)?;
        node.serialize_field("type", self.0.borrow().node_type())?;
        node.serialize_field("ports", &self.0.borrow().ports())?;
        node.end()
    }
}

#[cfg(test)]
mod test {
    // #[test]
    // #[ignore]
    // fn to_dot() {
    //     let node = OpticNode::new("Test", Dummy::default());
    //     assert_eq!(
    //         node.to_dot("i0", "".to_owned()).unwrap(),
    //         "  i0 [label=\"Test\"]\n".to_owned()
    //     )
    // }
    // #[test]
    // #[ignore]
    // fn to_dot_inverted() {
    //     let mut node = OpticNode::new("Test", Dummy::default());
    //     node.set_inverted(true);
    //     assert_eq!(
    //         node.to_dot("i0", "".to_owned()).unwrap(),
    //         "  i0 [label=\"Test(inv)\"]\n".to_owned()
    //     )
    // }
}
