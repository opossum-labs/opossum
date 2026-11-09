#![warn(missing_docs)]

use super::NodeGroup;
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::OpticNode,
    error::OpmResult,
    light::LightResult,
};

impl AnalysisRayTrace for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let ray_ends = config.collect_ray_ends().then_some(&mut self.ray_ends);
        self.graph
            .analyze_raytrace(&incoming_data, config, ray_ends)
    }

    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.graph
            .set_external_distances(self.input_port_distances.clone());
        let ray_ends = config.collect_ray_ends().then_some(&mut self.ray_ends);
        let result = self
            .graph
            .calc_node_positions(&incoming_data, config, ray_ends);
        self.reset_data();
        result
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::Analyzer,
        analyzers::RayTraceConfig,
        analyzers::raytrace::{AnalysisRayTrace, RayTracingAnalyzer},
        core_optics::{Alignable, node_attr::NodePositioning},
        degree,
        error::OpmResult,
        joule,
        light::LightResult,
        millimeter,
        nodes::{
            BeamSplitter, Dummy, NodeReference, SourcePort, ThinMirror,
            round_collimated_ray_builder,
        },
        utils::geom_transformation::Isometry,
    };
    use uuid::Uuid;

    /// Build a minimal Source → Dummy group with a configured source.
    ///
    /// Returns `(group, source_uuid)`. The Dummy's `output_1` is left unconnected so
    /// that, when `collect_ray_ends` is on, exactly one ray end bundle is produced.
    /// Both nodes carry an absolute identity isometry so `AnalysisRayTrace::analyze`
    /// can be called directly without a prior positioning walk.
    fn setup_source_dummy() -> OpmResult<(NodeGroup, Uuid)> {
        let mut group = NodeGroup::default();
        let i_src = group.add_node(SourcePort::default())?;
        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let i_dummy = group.add_node(dummy)?;
        group.connect_nodes(i_src, "output_1", i_dummy, "input_1", millimeter!(50.0))?;
        Ok((group, i_src))
    }

    fn config_with_flag(src_id: Uuid, collect: bool) -> OpmResult<RayTraceConfig> {
        let mut config = RayTraceConfig::default();
        config.map_source(
            src_id,
            round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 3)?,
        );
        config.set_collect_ray_ends(collect);
        Ok(config)
    }

    #[test]
    fn ray_ends_collected_after_analyze_with_flag() -> OpmResult<()> {
        let (mut group, src_id) = setup_source_dummy()?;
        let config = config_with_flag(src_id, true)?;
        AnalysisRayTrace::analyze(&mut group, LightResult::default(), &config)?;
        let ends = group.ray_ends();
        assert_eq!(ends.len(), 1, "Expected exactly one ray end bundle");
        assert!(
            ends[0].nr_of_rays(false) > 0,
            "Ray end bundle should contain rays"
        );
        // Each ray should carry position history from at least source + current position
        let first_ray = ends[0].iter().next().expect("bundle has at least one ray");
        assert!(
            first_ray.position_history_with_current().nrows() > 1,
            "Ray should have position history of at least 2 points"
        );
        Ok(())
    }

    #[test]
    fn ray_ends_empty_after_analyze_without_flag() -> OpmResult<()> {
        let (mut group, src_id) = setup_source_dummy()?;
        let config = config_with_flag(src_id, false)?;
        AnalysisRayTrace::analyze(&mut group, LightResult::default(), &config)?;
        assert!(
            group.ray_ends().is_empty(),
            "ray_ends() must be empty when collect_ray_ends is false"
        );
        Ok(())
    }

    #[test]
    fn ray_ends_present_after_positioning_with_flag() -> OpmResult<()> {
        let (mut group, src_id) = setup_source_dummy()?;
        let config = config_with_flag(src_id, true)?;
        // calc_node_positions calls reset_data() internally at its end — ray_ends must survive
        AnalysisRayTrace::calc_node_positions(&mut group, LightResult::default(), &config)?;
        let ends = group.ray_ends();
        assert_eq!(
            ends.len(),
            1,
            "Expected exactly one positioning ray end (should survive the internal reset_data)"
        );
        assert!(
            ends[0].nr_of_rays(false) > 0,
            "Positioning ray end bundle should contain at least one ray"
        );
        Ok(())
    }

    #[test]
    fn ray_ends_empty_after_positioning_without_flag() -> OpmResult<()> {
        let (mut group, src_id) = setup_source_dummy()?;
        let config = config_with_flag(src_id, false)?;
        AnalysisRayTrace::calc_node_positions(&mut group, LightResult::default(), &config)?;
        assert!(
            group.ray_ends().is_empty(),
            "ray_ends() must be empty when collect_ray_ends is false"
        );
        Ok(())
    }

    #[test]
    fn ray_ends_accumulate_over_repeated_walks() -> OpmResult<()> {
        let (mut group, src_id) = setup_source_dummy()?;
        let config = config_with_flag(src_id, true)?;
        AnalysisRayTrace::analyze(&mut group, LightResult::default(), &config)?;
        AnalysisRayTrace::analyze(&mut group, LightResult::default(), &config)?;
        assert_eq!(group.ray_ends().len(), 2);
        Ok(())
    }

    /// Ray-traces `model` the way the analyzer does: a positioning run (which never collects)
    /// followed by the trace itself, with ray ends collected.
    fn trace(model: &mut NodeGroup, source: Uuid) -> OpmResult<()> {
        RayTracingAnalyzer::new(config_with_flag(source, true)?).analyze(model)
    }

    /// Runs only the positioning walk over `model`, with ray ends collected.
    fn position(model: &mut NodeGroup, source: Uuid) -> OpmResult<()> {
        let config = config_with_flag(source, true)?;
        AnalysisRayTrace::calc_node_positions(model, LightResult::default(), &config)?;
        Ok(())
    }

    /// The number of ray ends the nested group `id` of `outer` holds itself, not counting
    /// groups nested further in.
    fn own_ends_of(outer: &NodeGroup, id: Uuid) -> Option<usize> {
        outer
            .nodes()
            .into_iter()
            .find(|node| node.uuid() == id)
            .and_then(|node| node.as_any().downcast_ref::<NodeGroup>())
            .map(|group| group.ray_ends.len())
    }

    /// A group passing light straight through one dummy, both of whose ports are mapped outward.
    fn pass_through_group() -> OpmResult<NodeGroup> {
        let mut group = NodeGroup::new("pass through");
        let dummy = group.add_node(Dummy::default())?;
        group.map_input_port(dummy, "input_1", "input_1")?;
        group.map_output_port(dummy, "output_1", "output_1")?;
        Ok(group)
    }

    /// A group holding a beam splitter: its first input and its first output are mapped outward,
    /// its second output is left open inside the group.
    fn splitting_group() -> OpmResult<NodeGroup> {
        let mut group = NodeGroup::new("splitting");
        let splitter = group.add_node(BeamSplitter::default())?;
        group.map_input_port(splitter, "input_1", "input_1")?;
        group.map_output_port(splitter, "out1_trans1_refl2", "output_1")?;
        Ok(group)
    }

    /// A source feeding `group`, whose output is left open. An inverted group is entered through
    /// the port it normally leaves by.
    ///
    /// Returns `(model, source, group)`.
    fn source_into(mut group: NodeGroup, inverted: bool) -> OpmResult<(NodeGroup, Uuid, Uuid)> {
        group.set_inverted(inverted)?;
        let entry = if inverted { "output_1" } else { "input_1" };
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let group = model.add_node(group)?;
        model.connect_nodes(source, "output_1", group, entry, millimeter!(50.0))?;
        Ok((model, source, group))
    }

    #[test]
    fn a_mapped_inner_port_ends_in_the_parent_only() -> OpmResult<()> {
        for walk in [trace, position] {
            let (mut model, source, group) = source_into(pass_through_group()?, false)?;
            walk(&mut model, source)?;
            assert_eq!(model.ray_ends.len(), 1);
            assert_eq!(own_ends_of(&model, group), Some(0));
        }
        Ok(())
    }

    #[test]
    fn an_unmapped_inner_port_ends_in_the_inner_group() -> OpmResult<()> {
        for walk in [trace, position] {
            let (mut model, source, group) = source_into(splitting_group()?, false)?;
            walk(&mut model, source)?;
            assert_eq!(
                model.ray_ends.len(),
                1,
                "the mapped output ends in the parent"
            );
            assert_eq!(own_ends_of(&model, group), Some(1), "the open one inside");
        }
        Ok(())
    }

    #[test]
    fn every_open_output_of_a_splitter_is_an_end() -> OpmResult<()> {
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let splitter = model.add_node(BeamSplitter::default())?;
        let dummy = model.add_node(Dummy::default())?;
        model.connect_nodes(source, "output_1", splitter, "input_1", millimeter!(50.0))?;
        model.connect_nodes(
            splitter,
            "out1_trans1_refl2",
            dummy,
            "input_1",
            millimeter!(50.0),
        )?;
        trace(&mut model, source)?;
        assert_eq!(model.ray_ends().len(), 2);
        Ok(())
    }

    #[test]
    fn an_inverted_node_ends_at_its_logical_output() -> OpmResult<()> {
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let mut dummy = Dummy::default();
        dummy.set_inverted(true)?;
        let dummy = model.add_node(dummy)?;
        model.connect_nodes(source, "output_1", dummy, "output_1", millimeter!(50.0))?;
        trace(&mut model, source)?;
        assert_eq!(model.ray_ends().len(), 1);
        Ok(())
    }

    #[test]
    fn an_inverted_group_tells_mapped_from_open_ports() -> OpmResult<()> {
        let (mut model, source, group) = source_into(pass_through_group()?, true)?;
        trace(&mut model, source)?;
        assert_eq!(model.ray_ends.len(), 1);
        assert_eq!(own_ends_of(&model, group), Some(0));

        let (mut model, source, group) = source_into(splitting_group()?, true)?;
        trace(&mut model, source)?;
        assert_eq!(
            model.ray_ends.len(),
            1,
            "the mapped output ends in the parent"
        );
        assert_eq!(own_ends_of(&model, group), Some(1), "the open one inside");
        Ok(())
    }

    /// A group passed twice in one run, the second time through an inverted reference behind a
    /// folding mirror: its open inner port is an end on both passes, so neither pass may wipe the
    /// other's.
    #[test]
    fn a_group_passed_twice_keeps_the_ends_of_both_passes() -> OpmResult<()> {
        let mut model = NodeGroup::default();
        let source = model.add_node(SourcePort::default())?;
        let group = model.add_node(splitting_group()?)?;
        let fold = model.add_node(ThinMirror::new("fold").with_tilt(degree!(2.0, 0.0, 0.0))?)?;
        let mut second_pass = NodeReference::from_node(&model.node(group)?)?;
        second_pass.set_inverted(true)?;
        let second_pass = model.add_node(second_pass)?;
        model.connect_nodes(source, "output_1", group, "input_1", millimeter!(30.0))?;
        model.connect_nodes(group, "output_1", fold, "input_1", millimeter!(50.0))?;
        model.connect_nodes(fold, "output_1", second_pass, "output_1", millimeter!(50.0))?;
        trace(&mut model, source)?;
        assert_eq!(own_ends_of(&model, group), Some(2));
        assert_eq!(
            model.ray_ends.len(),
            1,
            "the second pass leaves the model open"
        );
        Ok(())
    }
}
