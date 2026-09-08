use crate::{
    analyzers::{
        RayTraceConfig,
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, NodeAttrExt, OpticNode, OpticPorts, OpticRef, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    light::LightResult,
    nodes::NodeRegistration,
    properties::Proptype,
    utils::geom_transformation::Isometry,
};
use opm_macros_lib::OpmNode;
use uuid::Uuid;

inventory::submit! {
    NodeRegistration::new::<NodeReference>("reference", "reference to another node")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("lightsalmon3")]
#[manual_analyzable]
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
/// ## Properties
///   - `name`
///   - `inverted`
///
/// **Note**: Since this node only refers to another optical node it does not handle
/// (ignores) any [`Aperture`](crate::apertures::Aperture) definitions on its ports.
pub struct NodeReference {
    node_attr: NodeAttr,
    referenced_uuid: Uuid,
    ports: OpticPorts,
}

impl Analyzable for NodeReference {
    fn clone_analyzable(&self) -> Box<dyn Analyzable> {
        Box::new(self.clone())
    }

    fn referenced_node_id(&self) -> Option<Uuid> {
        Some(self.referenced_uuid)
    }
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
            node_attr,
            referenced_uuid: Uuid::nil(),
            ports: OpticPorts::default(),
        }
    }
}

impl NodeReference {
    /// Return the [`Uuid`] of the referenced optical node.
    pub fn referenced_uuid(&self) -> Uuid {
        self.referenced_uuid
    }

    /// Create a new [`NodeReference`] referring to another optical node.
    /// # Attributes
    /// `node`: `OpticRef` of the node
    ///
    /// # Errors
    ///
    /// This function returns an error if the internal reference id cannot be assigned.
    pub fn from_node(node: &OpticRef) -> OpmResult<Self> {
        let mut refr = Self::default();
        let target_uuid = node.node_attr().uuid();
        refr.referenced_uuid = target_uuid;
        refr.node_attr
            .set_property("reference id", Proptype::Uuid(target_uuid))?;
        let ref_name = format!("ref ({})", node.name());
        refr.node_attr.set_name(&ref_name);
        refr.ports = node.ports();
        if let Some(iso) = node.isometry() {
            refr.node_attr_mut().set_isometry(iso);
        }
        Ok(refr)
    }

    /// Assign a reference to another optical node.
    ///
    /// This functions allows for setting the optical node this [`NodeReference`] refers to.
    ///
    /// # Errors
    ///
    /// This function could return an error if an internal `reference_id` property cannot be assigned.
    pub fn assign_reference(&mut self, node: &OpticRef) -> OpmResult<()> {
        let target_uuid = node.uuid()?;
        self.referenced_uuid = target_uuid;
        self.node_attr_mut()
            .set_property("reference id", Proptype::Uuid(target_uuid))?;
        let ref_name = format!("ref ({})", node.name());
        self.node_attr.set_name(&ref_name);
        self.ports = node.ports();
        if let Some(iso) = node.isometry() {
            self.node_attr_mut().set_isometry(iso);
        }
        Ok(())
    }

    /// Update the cached ports from the referenced node.
    pub fn set_ports(&mut self, ports: OpticPorts) {
        self.ports = ports;
    }
}

impl OpticNode for NodeReference {
    fn ports(&self) -> OpticPorts {
        let mut ports = self.ports.clone();
        if self.inverted() {
            ports.set_inverted(true);
        }
        ports
    }

    fn isometry(&self) -> Option<Isometry> {
        self.node_attr().isometry()
    }

    fn set_isometry(
        &mut self,
        isometry: crate::utils::geom_transformation::Isometry,
    ) -> OpmResult<()> {
        self.node_attr_mut().set_isometry(isometry);
        Ok(())
    }

    fn update_surfaces(&mut self) -> OpmResult<()> {
        Ok(())
    }
}

impl AnalysisGhostFocus for NodeReference {}

impl AnalysisEnergy for NodeReference {
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        _config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        Err(OpossumError::Analysis(
            "NodeReference cannot be analyzed in isolation without a graph context".into(),
        ))
    }
}

impl AnalysisRayTrace for NodeReference {
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        _config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        Err(OpossumError::Analysis(
            "NodeReference cannot be analyzed in isolation without a graph context".into(),
        ))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        core_optics::PortType,
        nodes::{Dummy, NodeGroup, test_helper::test_helper::*},
    };

    #[test]
    fn default() {
        let node = NodeReference::default();
        assert_eq!(node.referenced_uuid(), Uuid::nil());
        assert_eq!(node.name(), "reference");
        assert_eq!(node.node_type(), "reference");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightsalmon3");
    }

    #[test]
    fn from_node() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(Dummy::default())?;
        let node_ref = scenery.node(node_id)?;
        let node = NodeReference::from_node(&node_ref)?;
        assert_eq!(node.referenced_uuid(), node_id);
        Ok(())
    }

    #[test]
    fn from_node_name() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(Dummy::default())?;
        let node_ref = scenery.node(node_id)?;
        let node_name = format!("ref ({})", node_ref.name());
        let node = NodeReference::from_node(&node_ref)?;
        assert_eq!(node.name(), node_name);
        Ok(())
    }

    #[test]
    fn assign_reference() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(Dummy::default())?;
        let node_ref = scenery.node(node_id)?;
        let mut node = NodeReference::default();
        assert_eq!(node.referenced_uuid(), Uuid::nil());
        node.assign_reference(&node_ref)?;
        assert_eq!(node.referenced_uuid(), node_id);
        Ok(())
    }

    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<NodeReference>()
    }

    #[test]
    fn ports_empty() {
        let node = NodeReference::default();
        assert!(node.ports().names(&PortType::Input).is_empty());
        assert!(node.ports().names(&PortType::Output).is_empty());
    }

    #[test]
    fn ports_non_empty() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(Dummy::default())?;
        let node = NodeReference::from_node(&scenery.node(node_id)?)?;
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
        Ok(())
    }

    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let node_id = scenery.add_node(Dummy::default())?;
        let mut node = NodeReference::from_node(&scenery.node(node_id)?)?;
        node.set_inverted(true.into())?;
        assert_eq!(node.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }
}
