use std::cell::RefCell;
use std::rc::{Rc, Weak};

use uuid::Uuid;

use crate::analyzer::AnalyzerType;
use crate::dottable::Dottable;
use crate::error::{OpmResult, OpossumError};
use crate::optic_ports::OpticPorts;
use crate::optic_ref::OpticRef;
use crate::optical::{LightResult, Optical};
use crate::properties::{PropCondition, Properties, Proptype, OpticalProperty};

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
    props
        .create(
            "name",
            "name of the reference node",
            Some(vec![PropCondition::NonEmptyString]),
            "reference".into(),
        )
        .unwrap();
    props
        .create(
            "node_type",
            "specific optical type of this node",
            Some(vec![PropCondition::NonEmptyString]),
            "reference".into(),
        )
        .unwrap();
    props
        .create("inverted", "inverse propagation?", None, false.into())
        .unwrap();
    props
        .create(
            "reference id",
            "unique id of the referenced node",
            None,
            Uuid::nil().into(),
        )
        .unwrap();
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
    /// Create a new [`NodeReference`] referring to another optical node.
    pub fn from_node(node: OpticRef) -> Self {
        let mut props = create_default_props();
        props.set("reference id", node.uuid().into()).unwrap();
        let ref_name = format!("ref ({})", node.optical_ref.borrow().name());
        props.set("name", Proptype::String(ref_name)).unwrap();
        Self {
            reference: Some(Rc::downgrade(&node.optical_ref)),
            props,
        }
    }
    pub fn assign_reference(&mut self, node: OpticRef) {
        self.reference = Some(Rc::downgrade(&node.optical_ref));
    }
}

impl Optical for NodeReference {
    fn ports(&self) -> OpticPorts {
        if let Some(rf) = &self.reference {
            let mut ports = rf.upgrade().unwrap().borrow().ports().clone();
            if self.properties().inverted() {
                ports.set_inverted(true);
            }
            ports
        } else {
            OpticPorts::default()
        }
    }
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let rf = &self
            .reference
            .clone()
            .ok_or(OpossumError::Analysis("no reference defined".into()))?;
        let ref_node = rf.upgrade().unwrap();
        let mut ref_node = ref_node.borrow_mut();
        if self.properties().inverted() {
            ref_node
                .set_property("inverted", true.into())
                .map_err(|_e| {
                    OpossumError::Analysis(format!(
                        "referenced node {} <{}> cannot be inverted",
                        ref_node.properties().name().unwrap(),
                        ref_node.properties().node_type().unwrap()
                    ))
                })?;
        }
        let output = ref_node.analyze(incoming_data, analyzer_type);
        if self.properties().inverted() {
            ref_node.set_property("inverted", false.into())?;
        }
        output
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
    fn as_refnode_mut(&mut self) -> OpmResult<&mut NodeReference> {
        Ok(self)
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
    use crate::{
        lightdata::{DataEnergy, LightData},
        nodes::{Dummy, Source},
        spectrum::create_he_ne_spectrum,
        OpticScenery,
    };
    #[test]
    fn default() {
        let node = NodeReference::default();
        assert!(node.reference.is_none());
        assert_eq!(node.properties().name().unwrap(), "reference");
        assert_eq!(node.properties().node_type().unwrap(), "reference");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted(), false);
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
    fn from_node_name() {
        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Dummy::default());
        let node_ref = scenery.node(idx).unwrap();
        let node_name = format!("ref ({})", node_ref.optical_ref.borrow().name());
        let node = NodeReference::from_node(node_ref);

        assert_eq!(node.name(), node_name);
    }
    #[test]
    fn assign_reference() {
        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Dummy::default());
        let node_ref = scenery.node(idx).unwrap();
        let mut node = NodeReference::default();
        assert!(node.reference.is_none());
        node.assign_reference(node_ref);
        assert!(node.reference.is_some());
    }
    #[test]
    fn inverted() {
        let mut node = NodeReference::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted(), true)
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
    #[test]
    fn ports_inverted() {
        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Dummy::default());
        let mut node = NodeReference::from_node(scenery.node(idx).unwrap());
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.ports().inputs(), vec!["rear"]);
        assert_eq!(node.ports().outputs(), vec!["front"]);
    }
    #[test]
    fn analyze() {
        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Dummy::default());
        let mut node = NodeReference::from_node(scenery.node(idx).unwrap());

        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("front".into(), Some(input_light.clone()));
        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("rear".into()));
        assert_eq!(output.len(), 1);
        let output = output.get("rear".into()).unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_inverse() {
        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Dummy::default());
        let mut node = NodeReference::from_node(scenery.node(idx).unwrap());
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("rear".into(), Some(input_light.clone()));

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("front".into()));
        assert_eq!(output.len(), 1);
        let output = output.get("front".into()).unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, input_light);
    }
    #[test]
    fn analyze_non_invertible_ref() {
        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Source::default());
        let mut node = NodeReference::from_node(scenery.node(idx).unwrap());
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        });
        input.insert("rear".into(), Some(input_light.clone()));

        let output = node.analyze(input, &AnalyzerType::Energy);
        assert!(output.is_err());
    }
}
