use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        RayTraceConfig,
    },
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    optic_node::OpticNode,
    optic_ports::OpticPorts,
    optic_ref::OpticRef,
    properties::Proptype,
    utils::geom_transformation::Isometry,
};
use opm_macros_lib::OpmNode;
use std::sync::{Arc, Mutex, Weak};
use uuid::Uuid;

#[derive(OpmNode, Debug, Clone)]
#[opm_node("lightsalmon3")]
/// A virtual component referring to another existing component.
///
/// This node type is necessary in order to model resonators (loops) or double-pass systems.
///
/// ## Optical Ports
///   - Inputs
///     - input ports of the referenced [`OpticNode`]
///   - Outputs
///     - output ports of the referenced [`OpticNode`]
///
/// ## Rpeoperties
///   - `name`
///   - `inverted`
///
/// **Note**: Since this node only refers to another optical node it does not handle
/// (ignores) any [`Aperture`](crate::aperture::Aperture) definitions on its ports.
pub struct NodeReference {
    reference: Option<Weak<Mutex<dyn Analyzable>>>,
    node_attr: NodeAttr,
}
impl Default for NodeReference {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("reference");
        node_attr
            .create_property(
                "reference id",
                "unique id of the referenced node",
                Uuid::nil().into(),
            )
            .unwrap();
        Self {
            reference: Option::default(),
            node_attr,
        }
    }
}
impl NodeReference {
    /// Create a new [`NodeReference`] referring to another optical node.
    /// # Attributes
    /// `node`: `OpticRef` of the node
    ///
    /// # Panics
    /// - if the node [`Properties`](crate::properties::Properties) `name` and `node_type` do not exist
    /// - if the node [`Properties`](crate::properties::Properties) `name` can not be set
    #[must_use]
    pub fn from_node(node: &OpticRef) -> Self {
        let mut refr = Self::default();
        let node_mut = node.optical_ref.lock().expect("Mutex lock failed");
        refr.node_attr
            .set_property("reference id", Proptype::Uuid(*node_mut.node_attr().uuid()))
            .unwrap();
        let ref_name = format!("ref ({})", node_mut.name());
        drop(node_mut);
        refr.node_attr.set_name(&ref_name);
        refr.reference = Some(Arc::downgrade(&node.optical_ref));
        refr
    }
    /// Assign a reference to another optical node.
    ///
    /// This functions allows for setting the optical node this [`NodeReference`] refers to. Normally, the [`OpticRef`] is given during the
    /// construction of a [`NodeReference`] using it's `new` function. This function allows for setting / changing after construction (e.g.
    /// during deserialization).
    pub fn assign_reference(&mut self, node: &OpticRef) {
        self.reference = Some(Arc::downgrade(&node.optical_ref));
    }
}
impl OpticNode for NodeReference {
    fn ports(&self) -> OpticPorts {
        self.reference
            .as_ref()
            .map_or_else(OpticPorts::default, |rf| {
                let mut ports = rf
                    .upgrade()
                    .unwrap()
                    .lock()
                    .expect("Mutex lock failed")
                    .ports();
                if self.inverted() {
                    ports.set_inverted(true);
                }
                ports
            })
    }
    fn as_refnode_mut(&mut self) -> OpmResult<&mut NodeReference> {
        Ok(self)
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn isometry(&self) -> Option<Isometry> {
        self.reference.as_ref().and_then(|rf| {
            rf.upgrade()
                .unwrap()
                .lock()
                .map_or(None, |ref_node| ref_node.isometry())
        })
    }
    fn set_isometry(
        &mut self,
        _isometry: crate::utils::geom_transformation::Isometry,
    ) -> OpmResult<()> {
        Ok(())
        // setting an isometry is silently ignored. Isometry is defined by the refrenced node.
    }

    fn update_surfaces(&mut self) -> OpmResult<()> {
        Ok(())
    }
}
impl AnalysisGhostFocus for NodeReference {}
impl AnalysisEnergy for NodeReference {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let rf = &self
            .reference
            .clone()
            .ok_or_else(|| OpossumError::Analysis("no reference defined".into()))?;
        let ref_node = rf.upgrade().unwrap();
        let mut ref_node = ref_node
            .lock()
            .map_err(|_| OpossumError::Analysis("Mutex lock failed".into()))?;
        if self.inverted() {
            ref_node.set_inverted(true).map_err(|_e| {
                OpossumError::Analysis(format!("referenced node {ref_node} cannot be inverted"))
            })?;
        }
        let output = AnalysisEnergy::analyze(&mut *ref_node, incoming_data);
        if self.inverted() {
            ref_node.set_inverted(false)?;
        }
        output
    }
}
unsafe impl Send for NodeReference {}

impl AnalysisRayTrace for NodeReference {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let rf = &self
            .reference
            .clone()
            .ok_or_else(|| OpossumError::Analysis("no reference defined".into()))?;
        let ref_node_arc = rf.upgrade().unwrap();
        let mut ref_node = ref_node_arc
            .lock()
            .map_err(|_| OpossumError::Analysis("Mutex lock failed".into()))?;
        if self.inverted() {
            ref_node.set_inverted(true).map_err(|_e| {
                OpossumError::Analysis(format!("referenced node {ref_node} cannot be inverted"))
            })?;
        }
        let output = AnalysisRayTrace::analyze(&mut *ref_node, incoming_data, config);
        if self.inverted() {
            ref_node.set_inverted(false)?;
        }
        output
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        lightdata::{DataEnergy, LightData},
        nodes::{test_helper::test_helper::*, Dummy, NodeGroup, Source},
        optic_ports::PortType,
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn default() {
        let mut node = NodeReference::default();
        assert!(node.reference.is_none());
        assert_eq!(node.name(), "reference");
        assert_eq!(node.node_type(), "reference");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightsalmon3");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn from_node() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Dummy::default()).unwrap();
        let node_ref = scenery.node(&node_id).unwrap();
        let node = NodeReference::from_node(&node_ref);
        assert!(node.reference.is_some());
    }
    #[test]
    fn from_node_name() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Dummy::default()).unwrap();
        let node_ref = scenery.node(&node_id).unwrap();
        let node_name = format!(
            "ref ({})",
            node_ref
                .optical_ref
                .lock()
                .expect("Mutex lock failed")
                .name()
        );
        let node = NodeReference::from_node(&node_ref);

        assert_eq!(node.name(), node_name);
    }
    #[test]
    fn assign_reference() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Dummy::default()).unwrap();
        let node_ref = scenery.node(&node_id).unwrap();
        let mut node = NodeReference::default();
        assert!(node.reference.is_none());
        node.assign_reference(&node_ref);
        assert!(node.reference.is_some());
    }
    #[test]
    fn inverted() {
        test_inverted::<NodeReference>()
    }
    #[test]
    fn ports_empty() {
        let node = NodeReference::default();
        assert!(node.ports().names(&PortType::Input).is_empty());
        assert!(node.ports().names(&PortType::Output).is_empty());
    }
    #[test]
    fn ports_non_empty() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Dummy::default()).unwrap();
        let node = NodeReference::from_node(&scenery.node(&node_id).unwrap());
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Dummy::default()).unwrap();
        let mut node = NodeReference::from_node(&scenery.node(&node_id).unwrap());
        node.set_inverted(true.into()).unwrap();
        assert_eq!(node.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["input_1"]);
    }
    #[test]
    fn analyze_empty() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Dummy::default()).unwrap();
        let mut node = NodeReference::from_node(&scenery.node(&node_id).unwrap());
        let output = AnalysisEnergy::analyze(&mut node, LightResult::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_no_reference() {
        let mut node = NodeReference::default();
        let output = AnalysisEnergy::analyze(&mut node, LightResult::default());
        assert!(output.is_err());
    }
    #[test]
    fn analyze() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Dummy::default()).unwrap();
        let mut node = NodeReference::from_node(&scenery.node(&node_id).unwrap());

        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_inverse() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Dummy::default()).unwrap();
        let mut node = NodeReference::from_node(&scenery.node(&node_id).unwrap());
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_non_invertible_ref() {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(&Source::default()).unwrap();
        let mut node = NodeReference::from_node(&scenery.node(&node_id).unwrap());
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input);
        assert!(output.is_err());
    }
}
