use crate::{
    analyzers::{
        RayTraceConfig,
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticPorts, OpticRef,
        node_attr::{HasNodeAttr, NodePositioning},
    },
    error::{OpmResult, OpossumError},
    light::LightResult,
    nodes::NodeRegistration,
    properties::Proptype,
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
    #[must_use]
    pub const fn referenced_uuid(&self) -> Uuid {
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
        refr.node_attr_mut().set_positioning(*node.positioning());
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
        let target_uuid = node.uuid();
        self.referenced_uuid = target_uuid;
        self.node_attr_mut()
            .set_property("reference id", Proptype::Uuid(target_uuid))?;
        let ref_name = format!("ref ({})", node.name());
        self.node_attr.set_name(&ref_name);
        self.ports = node.ports();
        self.node_attr_mut().set_positioning(*node.positioning());
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
    fn positioning(&self) -> &NodePositioning {
        self.node_attr().positioning()
    }
    fn set_positioning(&mut self, positioning: NodePositioning) -> OpmResult<()> {
        self.node_attr_mut().set_positioning(positioning);
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
    use tempfile::NamedTempFile;

    use super::*;
    use crate::{
        analyzers::AnalyzerType,
        core_optics::PortType,
        joule, millimeter,
        nodes::{
            Dummy, Lens, NodeGroup, RayPropagationVisualizer, SourcePort, ThinMirror,
            collimated_line_ray_builder, test_helper::test_helper::*,
        },
        opm_document::OpmDocument,
        refractive_index::RefrIndexConst,
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
    #[test]
    fn forward_reference_end_to_end_raytrace_and_serialization() -> OpmResult<()> {
        let mut scenery = NodeGroup::new("Forward Reference Scenery");

        // 1. Add SourcePort
        let src_id = scenery.add_node(SourcePort::default())?;

        // 2. Add real Lens (unplaced, without initial isometry)
        let refr_index = RefrIndexConst::new(1.5068)?;
        let lens = Lens::new(
            "Target Lens",
            millimeter!(150.0),
            millimeter!(-150.0),
            millimeter!(4.0),
            &refr_index,
        )?;
        let real_lens_id = scenery.add_node(lens)?;

        // Ensure the real lens is not positioned yet
        assert_eq!(
            scenery.node(real_lens_id)?.positioning(),
            &NodePositioning::Automatic(None)
        );

        // 3. Add NodeReference pointing to the unplaced lens
        let lens_proxy = NodeReference::from_node(&scenery.node(real_lens_id)?)?;
        let ref_id = scenery.add_node(lens_proxy)?;

        // 4. Add auxiliary components
        let mirror_id = scenery.add_node(ThinMirror::default())?;
        let visualizer_id = scenery.add_node(RayPropagationVisualizer::new("visualizer", None)?)?;

        // 5. Build topology: Source -> Reference -> Mirror -> Real Lens -> Visualizer
        scenery.connect_nodes(src_id, "output_1", ref_id, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(ref_id, "output_1", mirror_id, "input_1", millimeter!(40.0))?;
        scenery.connect_nodes(
            mirror_id,
            "output_1",
            real_lens_id,
            "input_1",
            millimeter!(40.0),
        )?;
        scenery.connect_nodes(
            real_lens_id,
            "output_1",
            visualizer_id,
            "input_1",
            millimeter!(50.0),
        )?;

        // 6. Create document and configure RayTrace analyzer
        let mut doc = OpmDocument::new(scenery);
        let ray_builder = collimated_line_ray_builder(millimeter!(10.0), joule!(1.0), 3)?;
        let mut config = RayTraceConfig::default();
        config.map_source(src_id, ray_builder);
        doc.add_analyzer(AnalyzerType::RayTrace(config));

        // 7. Verify save and reload functionality before analysis
        let temp_file = NamedTempFile::new()
            .map_err(|e| OpossumError::OpmDocument(format!("Temp file error: {e}")))?;
        doc.save_to_file(temp_file.path())?;

        let mut reloaded_doc = OpmDocument::from_file(temp_file.path())?;

        // 8. Run analysis on the reloaded document
        testing_logger::setup();
        reloaded_doc.analyze()?;

        // 9. Assertions:
        // The real lens must now have a valid isometry
        let real_lens = reloaded_doc.scenery().node(real_lens_id)?;
        let real_lens_iso = real_lens.positioning();
        let ref_node = reloaded_doc.scenery().node(ref_id)?;
        let ref_iso = ref_node.positioning();
        assert_eq!(
            ref_iso, real_lens_iso,
            "Reference and real lens must occupy the exact same spatial location"
        );

        Ok(())
    }
}
