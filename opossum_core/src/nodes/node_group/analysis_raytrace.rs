#![warn(missing_docs)]
use log::{info, warn};
use nalgebra::{Point3, Vector3};
use num_traits::Zero;
use petgraph::graph::NodeIndex;
use uom::si::f64::Length;
use uuid::Uuid;

use super::{NodeGroup, OpticGraph};
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, PortType, node_attr::NodePositioning},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult, Rays},
    radian,
    utils::geom_transformation::Isometry,
};

fn filter_ray_limits(light_result: &mut LightResult, r_config: &RayTraceConfig) {
    for lr in light_result {
        if let LightData::Geometric(rays) = lr.1 {
            rays.filter_by_nr_of_bounces(r_config.max_number_of_bounces());
            rays.filter_by_nr_of_refractions(r_config.max_number_of_refractions());
        }
    }
}

impl AnalysisRayTrace for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        if self.graph.is_inverted() {
            self.graph.invert_graph()?;
        }
        if !self.graph.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }
        let sorted = self.graph.topologically_sorted()?;
        let mut light_result = incoming_data.clone();
        for idx in sorted {
            let node_id = self.graph.g[idx].uuid();
            let node_info = format!("{}", self.graph.g[idx]);
            if self.graph.is_stale_node(node_id)? {
                warn!("graph contains stale (completely unconnected) node {node_info}. Skipping.");
            } else {
                let incoming_edges = self.graph.take_incoming(node_id, &incoming_data)?;
                let mut outgoing_edges = if let Some(target_uuid) =
                    self.graph.g[idx].referenced_node_id()
                {
                    let is_inverted = self.graph.g[idx].inverted();
                    let target_idx = self.graph.node_idx_by_uuid(target_uuid).ok_or_else(|| {
                        OpossumError::Analysis(format!(
                            "referenced node with id {target_uuid} not found in graph"
                        ))
                    })?;
                    let target_node = &mut self.graph.g[target_idx];
                    if is_inverted {
                        target_node.set_inverted(true).map_err(|_e| {
                            OpossumError::Analysis(format!(
                                "referenced node {target_node} cannot be inverted"
                            ))
                        })?;
                    }
                    let res = AnalysisRayTrace::analyze(&mut **target_node, incoming_edges, config);
                    if is_inverted {
                        target_node.set_inverted(false)?;
                    }
                    let node_info = format!("{target_node}");
                    res.map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_info} failed: {e}"))
                    })?
                } else {
                    let node = &mut self.graph.g[idx];
                    let node_info = format!("{node}");
                    AnalysisRayTrace::analyze(&mut **node, incoming_edges, config).map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_info} failed: {e}"))
                    })?
                };
                filter_ray_limits(&mut outgoing_edges, config);
                // If node is sink node, rewrite port names according to output mapping
                if self.graph.is_output_node(node_id)? {
                    let portmap = if self.graph.is_inverted() {
                        self.graph.port_map(&PortType::Input).clone()
                    } else {
                        self.graph.port_map(&PortType::Output).clone()
                    };
                    let assigned_ports = portmap.assigned_ports_for_node(node_id);
                    for port in assigned_ports {
                        if let Some(light_data) = outgoing_edges.get(&port.1) {
                            light_result.insert(port.0, light_data.clone());
                        }
                    }
                }
                for (port, data) in outgoing_edges {
                    let leftover = self.graph.set_outgoing_edge_data(idx, &port, data);
                    // Only an actual output port can be an end: a nested group returns its
                    // incoming data along with its outputs, so input-port keys show up here too.
                    if config.collect_ray_ends()
                        && let Some(LightData::Geometric(rays)) = leftover
                        && self.graph.g[idx]
                            .ports()
                            .names(&PortType::Output)
                            .contains(&port)
                        && !self.graph.is_mapped_output_port(node_id, &port)
                    {
                        self.ray_ends.push(rays);
                    }
                }
            }
        }
        if self.graph.is_inverted() {
            self.graph.invert_graph()?;
        } // revert initial inversion (if necessary)
        Ok(light_result)
    }

    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let collect = config.collect_ray_ends();

        // Set stored distances from predecessors
        self.graph
            .set_external_distances(self.input_port_distances.clone());

        let sorted = self.graph.topologically_sorted()?;
        let mut light_result = LightResult::default();
        let mut up_direction = Vector3::<f64>::y();

        for idx in sorted {
            let node_id = self
                .graph
                .node_by_idx(idx)
                .map_or_else(|_| Uuid::nil(), |node| node.uuid());

            let has_no_input_connections = !self.graph.has_input_connections(node_id)?;

            // A node is considered already placed if it either has an explicit absolute pose
            // or an already calculated automatic pose cached from a previous pass.
            let already_placed = match self.graph.g[idx].node_attr().positioning() {
                NodePositioning::Absolute(_) => true,
                NodePositioning::Automatic(cached) => cached.is_some(),
            };

            if has_no_input_connections && !already_placed {
                let node_info = format!("{}", self.graph.g[idx]);
                warn!(
                    "{node_info} has no incoming connections and can thus not being placed. Skipping."
                );
            } else {
                // Disjoint-field borrow: self.graph and self.ray_ends are separate fields.
                let ray_ends_opt: Option<&mut Vec<Rays>> = if collect {
                    Some(&mut self.ray_ends)
                } else {
                    None
                };
                calculate_single_node_position(
                    &mut self.graph,
                    idx,
                    &incoming_data,
                    &mut up_direction,
                    config,
                    &mut light_result,
                    ray_ends_opt,
                )?;
            }
        }

        self.reset_data();
        Ok(light_result)
    }
}

/// Helper function to position an individual node either via alignment or incoming beam.
fn position_node(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: &LightResult,
    up_direction: &Vector3<f64>,
) -> OpmResult<()> {
    let node_attr = graph.g[node_idx].node_attr().clone();
    let node_id = node_attr.uuid();

    if let Some((align_id, distance)) = node_attr.get_align_like_node_at_distance() {
        // Disentangle the borrow from graph by cloning the effective position immediately
        let align_ref_iso = graph
            .node(*align_id)?
            .positioning()
            .effective_position()
            .copied();

        if let Some(align_ref_iso) = align_ref_iso {
            let align_iso = Isometry::new(
                Point3::new(Length::zero(), Length::zero(), *distance),
                radian!(0., 0., 0.),
            )?;
            let new_iso = align_ref_iso.append(&align_iso);
            graph.g[node_idx].set_positioning(NodePositioning::Automatic(Some(new_iso)))?;
        } else {
            warn!(
                "Cannot align node like NodeIdx:{}. Fall back to standard positioning method",
                node_idx.index()
            );
            graph.set_node_isometry(incoming_edges, *align_id, *up_direction)?;
        }
    } else {
        graph.set_node_isometry(incoming_edges, node_id, *up_direction)?;
    }

    Ok(())
}

/// Ensures that the node at `node_idx` has a valid spatial isometry assigned.
///
/// Supports both forward and backward node references by synchronizing the spatial
/// isometry bidirectionally between reference proxies and their target components.
fn ensure_node_isometry(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: &LightResult,
    up_direction: &Vector3<f64>,
) -> OpmResult<()> {
    let node_attr = graph.g[node_idx].node_attr().clone();
    let node_info = format!("{}", graph.g[node_idx]);

    // Fast-path: Check positioning status
    match node_attr.positioning() {
        NodePositioning::Absolute(_) => {
            info!("Node {node_info} has an absolute position. Leaving untouched.");
            return Ok(());
        }
        NodePositioning::Automatic(Some(_)) => {
            info!("Node {node_info} has already been placed. Leaving untouched.");
            return Ok(());
        }
        NodePositioning::Automatic(None) => {
            // Node needs placement along the optical axis; proceed below.
        }
    }

    // Check if current node is a reference proxy
    if let Some(target_uuid) = graph.g[node_idx].referenced_node_id() {
        let target_idx = graph.node_idx_by_uuid(target_uuid).ok_or_else(|| {
            OpossumError::Analysis(format!(
                "referenced node with id {target_uuid} not found in graph"
            ))
        })?;

        // Case 1: Backward reference (target node was already placed earlier).
        // Clone the effective position upfront to end the immutable borrow on graph.g before mutating.
        let target_iso = graph.g[target_idx]
            .positioning()
            .effective_position()
            .copied();

        if let Some(target_iso) = target_iso {
            info!("Target node for reference {node_info} is already placed. Adopting isometry.");
            graph.g[node_idx].set_positioning(NodePositioning::Automatic(Some(target_iso)))?;
            return Ok(());
        }

        // Case 2: Forward reference (reference is encountered before the target node).
        // Position the reference node using incoming beam data
        position_node(graph, node_idx, incoming_edges, up_direction)?;

        // Propagate the calculated isometry to the target node so its surfaces are updated.
        // Clone the calculated position upfront to prevent aliasing with &mut graph.g[target_idx].
        let calculated_iso = graph.g[node_idx]
            .positioning()
            .effective_position()
            .copied();

        if let Some(calculated_iso) = calculated_iso {
            let target_node = &mut graph.g[target_idx];
            info!(
                "Forward reference {node_info} positioned target node {}.",
                target_node.name()
            );
            target_node.set_positioning(NodePositioning::Automatic(Some(calculated_iso)))?;
        }

        return Ok(());
    }

    // Standard node positioning
    position_node(graph, node_idx, incoming_edges, up_direction)?;

    Ok(())
}

/// Executes ray-trace calculations to determine node positioning and outgoing beams.
///
/// Handles inversion states appropriately if the target node is a reference proxy.
fn execute_node_calculation(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: LightResult,
    config: &RayTraceConfig,
) -> OpmResult<LightResult> {
    if let Some(target_uuid) = graph.g[node_idx].referenced_node_id() {
        let is_inverted = graph.g[node_idx].inverted();
        let target_idx = graph.node_idx_by_uuid(target_uuid).ok_or_else(|| {
            OpossumError::Analysis(format!(
                "referenced node with id {target_uuid} not found in graph"
            ))
        })?;
        let target_node = &mut graph.g[target_idx];
        if is_inverted {
            target_node.set_inverted(true).map_err(|_e| {
                OpossumError::Analysis(format!("referenced node {target_node} cannot be inverted"))
            })?;
        }
        let res = AnalysisRayTrace::analyze(&mut **target_node, incoming_edges, config);
        if is_inverted {
            target_node.set_inverted(false)?;
        }
        res
    } else {
        let node = &mut graph.g[node_idx];
        AnalysisRayTrace::calc_node_positions(&mut **node, incoming_edges, config)
    }
}

/// Maps outgoing beam data from internal output nodes to external group ports.
fn record_output_ports(
    graph: &OpticGraph,
    node_id: Uuid,
    outgoing_edges: &LightResult,
    light_result: &mut LightResult,
) -> OpmResult<()> {
    if graph.is_output_node(node_id)? {
        let portmap = if graph.is_inverted() {
            graph.port_map(&PortType::Input).clone()
        } else {
            graph.port_map(&PortType::Output).clone()
        };
        let assigned_ports = portmap.assigned_ports_for_node(node_id);
        for port in assigned_ports {
            if let Some(light_data) = outgoing_edges.get(&port.1) {
                light_result.insert(port.0, light_data.clone());
            }
        }
    }
    Ok(())
}

/// Stores outgoing edge data in the graph and updates the up-direction reference vector.
///
/// When `ray_ends` is `Some`, ray bundles that are returned by [`OpticGraph::set_outgoing_edge_data`]
/// (i.e. bundles with no connected edge) are pushed into it, provided the port is not a mapped
/// external output port of the group.  When `ray_ends` is `None`, behavior is identical to the
/// previous (flag-off) path.
///
/// # Arguments
///
/// * `graph` - The optical graph to operate on.
/// * `node_idx` - Index of the node whose outgoing beams are being processed.
/// * `outgoing_edges` - The light data produced by the node for each of its output ports.
/// * `up_direction` - Running "up" direction vector, updated by this function.
/// * `ray_ends` - Optional collector for unconnected, unmapped ray bundles.
///
/// # Errors
///
/// This function returns an error if a referenced node cannot be found, if the up-direction cannot
/// be derived from an outgoing beam, or if an outgoing beam holds no rays at all: the optical axis
/// ends at this node then, and nothing connected after it can be placed.
fn update_outgoing_edges_and_up_direction(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    outgoing_edges: LightResult,
    up_direction: &mut Vector3<f64>,
    mut ray_ends: Option<&mut Vec<Rays>>,
) -> OpmResult<()> {
    let referenced_id = graph.g[node_idx].referenced_node_id();
    let fallback_node_type = graph.g[node_idx].node_attr().node_type().to_string();
    let node_id = graph.g[node_idx].uuid();

    for (port, data) in outgoing_edges {
        // Checked here rather than left to the up-direction calculation below, which could only
        // report an empty bundle - not where the optical axis was lost.
        if let LightData::Geometric(rays) = &data
            && rays.nr_of_rays(false) == 0
        {
            return Err(OpossumError::Analysis(format!(
                "no light leaves {} at port '{}': the optical axis ends there, so the components \
                 after it cannot be placed",
                graph.g[node_idx], port
            )));
        }
        if let Some(target_uuid) = referenced_id {
            let target_idx = graph.node_idx_by_uuid(target_uuid).ok_or_else(|| {
                OpossumError::Analysis(format!(
                    "referenced node with id {target_uuid} not found in graph"
                ))
            })?;
            let target = &graph.g[target_idx];
            if target.node_type() == "source" || target.node_type() == "source port" {
                *up_direction = target.define_up_direction(&data)?;
            } else {
                target.calc_new_up_direction(&data, up_direction)?;
            }
        } else {
            let node = &graph.g[node_idx];
            if fallback_node_type == "source" || fallback_node_type == "source port" {
                *up_direction = node.define_up_direction(&data)?;
            } else {
                node.calc_new_up_direction(&data, up_direction)?;
            }
        }
        let leftover = graph.set_outgoing_edge_data(node_idx, &port, data);
        if let Some(ray_ends) = ray_ends.as_deref_mut()
            && let Some(LightData::Geometric(rays)) = leftover
            && !graph.is_mapped_output_port(node_id, &port)
        {
            ray_ends.push(rays);
        }
    }
    Ok(())
}

fn calculate_single_node_position(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_data: &LightResult,
    up_direction: &mut Vector3<f64>,
    config: &RayTraceConfig,
    light_result: &mut LightResult,
    ray_ends: Option<&mut Vec<Rays>>,
) -> OpmResult<()> {
    let node_id = graph.g[node_idx].uuid();
    let node_info = format!("{}", graph.g[node_idx]);
    let incoming_edges: LightResult = graph.get_incoming(node_id, incoming_data)?;

    // 1. Position the node if not already placed
    ensure_node_isometry(graph, node_idx, &incoming_edges, up_direction)?;

    // 2. Compute the optical calculation / ray tracing
    let output = execute_node_calculation(graph, node_idx, incoming_edges, config);

    // 3. Handle potential calculation errors on non-terminal vs terminal nodes
    let outgoing_edges = match output {
        Ok(edges) => edges,
        Err(e) => {
            if graph.has_output_connections(node_id)? {
                return Err(OpossumError::Analysis(format!(
                    "calculation of optical axis for node {node_info} failed: {e}"
                )));
            }
            warn!(
                "Calculation of optical axis for terminal node {node_info} failed: {e}. Ignoring as it has no successors."
            );
            LightResult::default()
        }
    };

    // 4. Map output ports for group-level sinks
    record_output_ports(graph, node_id, &outgoing_edges, light_result)?;

    // 5. Propagate outgoing beam data and adjust the up vector
    update_outgoing_edges_and_up_direction(
        graph,
        node_idx,
        outgoing_edges,
        up_direction,
        ray_ends,
    )?;

    Ok(())
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
