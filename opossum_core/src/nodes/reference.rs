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
    geometry::Geometry,
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
    /// A reference has no shape of its own; it is traced through the node it refers to.
    fn geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(None)
    }
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
        analyzers::{
            Analyzer, AnalyzerType, GhostFocusConfig, energy::EnergyAnalyzer,
            ghostfocus::GhostFocusAnalyzer, raytrace::RayTracingAnalyzer,
        },
        core_optics::{Alignable, PortType},
        degree, joule,
        light::{LightData, Rays, lightdata::energy_data_builder::EnergyDataBuilder},
        millimeter,
        nodes::{
            Dummy, EnergyMeter, Lens, NodeGroup, ParaxialSurface, RayPropagationVisualizer,
            SourcePort, ThinMirror, collimated_line_ray_builder, round_collimated_ray_builder,
            test_helper::helper::*,
        },
        opm_document::OpmDocument,
        refractive_index::RefrIndexConst,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;

    /// A double pass as in `workshop_09_phelix`: a lens, a mirror sending the light back through
    /// it at a slight angle, an inverted reference on the lens for the way back, and a meter.
    fn double_pass() -> OpmResult<[OpticRef; 4]> {
        let lens = OpticRef::new(Box::new(ParaxialSurface::new("lens", millimeter!(500.0))?));
        let mirror = OpticRef::new(Box::new(
            ThinMirror::new("mirror").with_tilt(degree!(1.0, 0.0, 0.0))?,
        ));
        let mut back = NodeReference::from_node(&lens)?;
        back.set_inverted(true)?;
        let meter = OpticRef::new(Box::new(EnergyMeter::default()));
        Ok([lens, mirror, OpticRef::new(Box::new(back)), meter])
    }
    /// A part of a model: the component itself, or a group holding only it, its ports mapped
    /// outward as `input_1` and `output_1`.
    fn part(node: &OpticRef, grouped: bool) -> OpmResult<OpticRef> {
        if !grouped {
            return Ok(node.clone());
        }
        let mut group = NodeGroup::new("group");
        let id = group.add_node_ref(node.clone())?;
        group.map_input_port(id, &node.ports().names(&PortType::Input)[0], "input_1")?;
        group.map_output_port(id, &node.ports().names(&PortType::Output)[0], "output_1")?;
        Ok(OpticRef::new(Box::new(group)))
    }
    /// A model of a source followed by `parts` in a row, 100 mm apart, each component alone or in
    /// a group of its own as `grouped` says.
    ///
    /// # Returns
    ///
    /// The model and the uuid of its source.
    fn model(parts: &[OpticRef], grouped: &[bool]) -> OpmResult<(NodeGroup, Uuid)> {
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let mut upstream = (source, "output_1".to_string());
        for (node, grouped) in parts.iter().zip(grouped) {
            let node = part(node, *grouped)?;
            let id = model.add_node_ref(node.clone())?;
            let input = node.ports().names(&PortType::Input)[0].clone();
            model.connect_nodes(upstream.0, &upstream.1, id, &input, millimeter!(100.0))?;
            upstream = (id, node.ports().names(&PortType::Output)[0].clone());
        }
        Ok((model, source))
    }
    /// The light the meter of `model` recorded.
    fn metered(model: &NodeGroup, meter: Uuid) -> OpmResult<LightData> {
        let (meter, _) = model.node_recursive(meter)?;
        meter
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .and_then(EnergyMeter::light_data)
            .cloned()
            .ok_or_else(|| OpossumError::Other("the meter recorded nothing".into()))
    }
    /// What a model of `parts` delivers in every analysis: where each part was placed, and what
    /// its meter, the last part, recorded in a ray trace, an energy flow and a ghost focus
    /// analysis.
    struct Outcome {
        placements: Vec<Isometry>,
        traced: LightData,
        energy: LightData,
        ghosts: LightData,
    }
    fn outcome(parts: &[OpticRef], grouped: &[bool]) -> OpmResult<Outcome> {
        let ids: Vec<Uuid> = parts.iter().map(OpticRef::uuid).collect();
        let meter = ids[ids.len() - 1];
        let rays = || round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 3);

        let (mut placed, source) = model(parts, grouped)?;
        let mut config = RayTraceConfig::default();
        config.map_source(source, rays()?);
        AnalysisRayTrace::calc_node_positions(
            &mut placed,
            LightResult::default(),
            &config.for_positioning(),
        )?;
        let placements = ids
            .iter()
            .map(|id| {
                placed
                    .node_recursive(*id)?
                    .0
                    .effective_position()
                    .copied()
                    .ok_or_else(|| OpossumError::Other(format!("node {id} was not placed")))
            })
            .collect::<OpmResult<_>>()?;

        let (mut traced, source) = model(parts, grouped)?;
        let mut config = RayTraceConfig::default();
        config.map_source(source, rays()?);
        RayTracingAnalyzer::new(config).analyze(&mut traced)?;

        let (mut flown, source) = model(parts, grouped)?;
        let mut config = EnergyConfig::default();
        config.map_source(source, EnergyDataBuilder::default());
        EnergyAnalyzer::new(config).analyze(&mut flown)?;

        let (mut ghosted, source) = model(parts, grouped)?;
        let mut config = GhostFocusConfig::default();
        config.set_max_bounces(1);
        config.map_source(source, rays()?);
        GhostFocusAnalyzer::new(config).analyze(&mut ghosted)?;

        Ok(Outcome {
            placements,
            traced: metered(&traced, meter)?,
            energy: metered(&flown, meter)?,
            ghosts: metered(&ghosted, meter)?,
        })
    }
    /// The rays in `light`, one bundle per entry.
    fn bundles(light: &LightData) -> Vec<Rays> {
        match light {
            LightData::Geometric(rays) => vec![rays.clone()],
            LightData::GhostFocus(bundles) => bundles.clone(),
            _ => Vec::new(),
        }
    }
    fn assert_same_outcome(expected: &Outcome, actual: &Outcome) {
        assert_eq!(expected.placements.len(), actual.placements.len());
        for (expected, actual) in expected.placements.iter().zip(&actual.placements) {
            assert_relative_eq!(
                expected.get_transform().to_homogeneous(),
                actual.get_transform().to_homogeneous(),
                epsilon = 1e-9
            );
        }
        for (expected, actual) in [
            (&expected.traced, &actual.traced),
            (&expected.ghosts, &actual.ghosts),
        ] {
            let (expected, actual) = (bundles(expected), bundles(actual));
            assert!(!expected.is_empty());
            assert_eq!(expected.len(), actual.len());
            for (expected, actual) in expected.iter().zip(&actual) {
                assert_ray_bundle_snapshot(
                    &ray_bundle_snapshot(actual),
                    &ray_bundle_snapshot(expected),
                );
            }
        }
        let (LightData::Energy(expected), LightData::Energy(actual)) =
            (&expected.energy, &actual.energy)
        else {
            panic!("the meter recorded no energy");
        };
        assert_relative_eq!(expected.total_energy(), actual.total_energy());
    }
    /// A reference finds the node it refers to inside a group, as when the optics before the main
    /// amplifiers of `workshop_09_phelix` are grouped: every analysis gives what the flat model
    /// gives, and the way back is placed where the lens is.
    #[test]
    fn a_reference_finds_its_target_in_a_group() -> OpmResult<()> {
        let parts = double_pass()?;
        let flat = outcome(&parts, &[false; 4])?;
        assert_relative_eq!(
            flat.placements[0].get_transform().to_homogeneous(),
            flat.placements[2].get_transform().to_homogeneous()
        );
        assert_same_outcome(&flat, &outcome(&parts, &[true, false, false, false])?);
        Ok(())
    }
    /// A reference met before the node it refers to places that node, also inside a group: the
    /// light passes the reference first, then a mirror sends it back through the lens itself.
    #[test]
    fn a_forward_reference_places_its_target_in_a_group() -> OpmResult<()> {
        let mut lens = ParaxialSurface::new("lens", millimeter!(500.0))?;
        let ahead = NodeReference::from_node(&OpticRef::new(Box::new(lens.clone())))?;
        lens.set_inverted(true)?;
        let mirror = ThinMirror::new("mirror").with_tilt(degree!(1.0, 0.0, 0.0))?;
        let parts = [
            OpticRef::new(Box::new(ahead)),
            OpticRef::new(Box::new(mirror)),
            OpticRef::new(Box::new(lens)),
            OpticRef::new(Box::new(EnergyMeter::default())),
        ];
        let flat = outcome(&parts, &[false; 4])?;
        assert_same_outcome(&flat, &outcome(&parts, &[false, false, true, false])?);
        Ok(())
    }

    #[test]
    fn default() {
        let node = NodeReference::default();
        assert_eq!(node.referenced_uuid(), Uuid::nil());
        assert_eq!(node.name(), "reference");
        assert_eq!(node.node_type(), "reference");
        assert!(!node.inverted());
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
        node.set_inverted(true)?;
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
            refr_index,
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

        // The mirror sends the light back through the real lens, which it passes backwards: the
        // lens is inverted for that pass, as any double pass is built.
        let mut inverted = Ok(());
        scenery.for_each_node_mut(&mut |node| {
            if node.uuid() == real_lens_id {
                inverted = node.set_inverted(true);
            }
        });
        inverted?;

        // 4. Add auxiliary components
        let mirror_id = scenery.add_node(ThinMirror::default())?;
        let visualizer_id = scenery.add_node(RayPropagationVisualizer::new("visualizer", None)?)?;

        // 5. Build topology: Source -> Reference -> Mirror -> Real Lens (backwards) -> Visualizer
        scenery.connect_nodes(src_id, "output_1", ref_id, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(ref_id, "output_1", mirror_id, "input_1", millimeter!(40.0))?;
        scenery.connect_nodes(
            mirror_id,
            "output_1",
            real_lens_id,
            "output_1",
            millimeter!(40.0),
        )?;
        scenery.connect_nodes(
            real_lens_id,
            "input_1",
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
