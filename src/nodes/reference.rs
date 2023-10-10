use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::optic_ports::OpticPorts;
use crate::optical::{LightResult, OpticRef, Optical};
use crate::properties::{Properties, Property, Proptype};

#[derive(Debug)]
/// A virtual component referring to another existing component.
///
/// This node type is necessary in order to model resonators (loops) or double-pass systems.
///
/// ## Optical Ports
///   - Inputs
///     - input ports of the referenced [`Optical`]
///   - Outputs
///     - output ports of the referenced [`Optical`]
///
/// ## Rpeoperties
///   - `name`
///   - `inverted`
pub struct NodeReference {
    reference: Option<Weak<RefCell<dyn Optical>>>,
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set("name", "reference".into());
    props.set("inverted", false.into());
    props
}
impl Default for NodeReference {
    fn default() -> Self {
        Self {
            reference: Default::default(),
            props: create_default_props(),
        }
    }
}
impl NodeReference {
    // Create new [`NodeReference`] referring to another existing [`OpticRef`].
    pub fn from_node(node: OpticRef) -> Self {
        Self {
            reference: Some(Rc::downgrade(&node.0)),
            props: create_default_props(),
        }
    }
}

impl Optical for NodeReference {
    fn name(&self) -> &str {
        if let Proptype::String(name) = &self.props.get("name").unwrap().prop {
            name
        } else {
            self.node_type()
        }
    }
    fn node_type(&self) -> &str {
        "reference"
    }
    fn inverted(&self) -> bool {
        self.properties().get_bool("inverted").unwrap().unwrap()
    }
    fn ports(&self) -> OpticPorts {
        if let Some(rf) = &self.reference {
            rf.upgrade().unwrap().borrow().ports().clone()
        } else {
            OpticPorts::default()
        }
    }

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let rf = &self.reference.clone().ok_or(OpossumError::Analysis(
            "reference node has no reference defined".into(),
        ))?;
        rf.upgrade()
            .unwrap()
            .borrow_mut()
            .analyze(incoming_data, analyzer_type)
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Property) -> OpmResult<()> {
        if self.props.set(name, prop).is_none() {
            Err(OpossumError::Other("property not defined".into()))
        } else {
            Ok(())
        }
    }
}

impl Dottable for NodeReference {
    fn node_color(&self) -> &str {
        "lightsalmon3"
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{nodes::Dummy, OpticScenery};
    #[test]
    fn default() {
        let node = NodeReference::default();
        assert!(node.reference.is_none());
        assert_eq!(node.name(), "reference");
        assert_eq!(node.node_type(), "reference");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightsalmon3");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn from_node() {
        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Dummy::default());
        let node_ref = scenery.node(idx).unwrap();
        let node = NodeReference::from_node(node_ref);
        assert!(node.reference.is_some());
    }
    #[test]
    fn inverted() {
        let mut node = NodeReference::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.inverted(), true)
    }
    #[test]
    fn ports_empty() {
        let node = NodeReference::default();
        assert!(node.ports().inputs().is_empty());
        assert!(node.ports().outputs().is_empty());
    }
    #[test]
    fn ports_non_empty() {
        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Dummy::default());
        let node = NodeReference::from_node(scenery.node(idx).unwrap());
        assert_eq!(node.ports().inputs(), vec!["front"]);
        assert_eq!(node.ports().outputs(), vec!["rear"]);
    }
}
