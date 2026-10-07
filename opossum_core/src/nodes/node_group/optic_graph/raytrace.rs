#![warn(missing_docs)]

use super::{InvertGraphGuard, OpticGraph};
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, OpticNodeExt, PortType, node_attr::NodePositioning},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult, Rays},
    radian,
    utils::geom_transformation::Isometry,
};
use log::{info, warn};
use nalgebra::{Point3, Vector3};
use num_traits::Zero;
use petgraph::graph::NodeIndex;
use uom::si::f64::Length;
use uuid::Uuid;

fn filter_ray_limits(light_result: &mut LightResult, r_config: &RayTraceConfig) {
    for lr in light_result {
        if let LightData::Geometric(rays) = lr.1 {
            rays.filter_by_nr_of_bounces(r_config.max_number_of_bounces());
            rays.filter_by_nr_of_refractions(r_config.max_number_of_refractions());
        }
    }
}

impl OpticGraph {
    /// Performs ray tracing analysis through the optical graph.
    ///
    /// # Arguments
    ///
    /// * `incoming_data` - the light entering the graph through its external input ports.
    /// * `config` - the ray tracing configuration.
    /// * `ray_ends` - if given, collects the ray bundles leaving through output ports that are
    ///   neither connected nor mapped outward.
    ///
    /// # Errors
    ///
    /// This function returns an error if underlying functions fail.
    pub fn analyze_raytrace(
        &mut self,
        incoming_data: &LightResult,
        config: &RayTraceConfig,
        mut ray_ends: Option<&mut Vec<Rays>>,
    ) -> OpmResult<LightResult> {
        let is_inverted = self.is_inverted();
        let mut guard = InvertGraphGuard::new(self, is_inverted)?;

        if !guard.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }

        let sorted = guard.topologically_sorted()?;
        let mut light_result = incoming_data.clone();

        for idx in sorted {
            let node_id = guard.g[idx].uuid();
            let node_info = format!("{}", guard.g[idx]);

            if guard.is_stale_node(node_id)? {
                warn!("graph contains stale (completely unconnected) node {node_info}. Skipping.");
            } else {
                let incoming_edges = guard.take_incoming(node_id, incoming_data)?;

                // Evaluate node or reference proxy using shared helper
                let mut outgoing_edges = guard.evaluate_node_or_reference(idx, |node| {
                    AnalysisRayTrace::analyze(&mut **node, incoming_edges, config)
                })?;

                filter_ray_limits(&mut outgoing_edges, config);

                // Map outgoing ports to external group ports
                guard.collect_group_output_ports(node_id, &outgoing_edges, &mut light_result)?;

                for (port, data) in outgoing_edges {
                    let leftover = guard.set_outgoing_edge_data(idx, &port, data);
                    // Only an actual output port can be an end: a nested group returns its
                    // incoming data along with its outputs, so input-port keys show up here too.
                    if let Some(ray_ends) = ray_ends.as_deref_mut()
                        && let Some(LightData::Geometric(rays)) = leftover
                        && guard.g[idx]
                            .ports()
                            .names(&PortType::Output)
                            .contains(&port)
                        && !guard.is_mapped_output_port(node_id, &port)
                    {
                        ray_ends.push(rays);
                    }
                }
            }
        }

        Ok(light_result)
    }

    /// Positions nodes in the graph based on optical ray propagation.
    ///
    /// # Arguments
    ///
    /// * `incoming_data` - the optical axis entering the graph through its external input ports.
    /// * `config` - the ray tracing configuration of the positioning run.
    /// * `ray_ends` - if given, collects the ray bundles leaving through output ports that are
    ///   neither connected nor mapped outward.
    ///
    /// # Errors
    ///
    /// This function returns an error if the node positions could not be determined (e.g. optical axis misses element
    /// or "gets lost").
    pub fn calc_node_positions(
        &mut self,
        incoming_data: &LightResult,
        config: &RayTraceConfig,
        mut ray_ends: Option<&mut Vec<Rays>>,
    ) -> OpmResult<LightResult> {
        let sorted = self.topologically_sorted()?;
        let mut light_result = LightResult::default();
        let mut up_direction = Vector3::<f64>::y();

        for idx in sorted {
            let node_id = self
                .node_by_idx(idx)
                .map_or_else(|_| Uuid::nil(), |node| node.uuid());
            let has_no_input_connections = !self.has_input_connections(node_id)?;

            let already_placed = match self.g[idx].node_attr().positioning() {
                NodePositioning::Absolute(_) => true,
                NodePositioning::Automatic(cached) => cached.is_some(),
            };

            if has_no_input_connections && !already_placed {
                let node_info = format!("{}", self.g[idx]);
                warn!(
                    "{node_info} has no incoming connections and can thus not being placed. Skipping."
                );
            } else {
                calculate_single_node_position(
                    self,
                    idx,
                    incoming_data,
                    &mut up_direction,
                    config,
                    &mut light_result,
                    ray_ends.as_deref_mut(),
                )?;
            }
        }

        Ok(light_result)
    }
}

fn position_node(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: &LightResult,
    up_direction: &Vector3<f64>,
) -> OpmResult<()> {
    let node_attr = graph.g[node_idx].node_attr().clone();
    let node_id = node_attr.uuid();

    if let Some((align_id, distance)) = node_attr.get_align_like_node_at_distance() {
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

fn ensure_node_isometry(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: &LightResult,
    up_direction: &Vector3<f64>,
) -> OpmResult<()> {
    let node_attr = graph.g[node_idx].node_attr().clone();
    let node_info = format!("{}", graph.g[node_idx]);

    match node_attr.positioning() {
        NodePositioning::Absolute(_) => {
            info!("Node {node_info} has an absolute position. Leaving untouched.");
            return Ok(());
        }
        NodePositioning::Automatic(Some(_)) => {
            info!("Node {node_info} has already been placed. Leaving untouched.");
            return Ok(());
        }
        NodePositioning::Automatic(None) => {}
    }

    if let Some(target_uuid) = graph.g[node_idx].referenced_node_id() {
        let target_idx = graph.node_idx_by_uuid(target_uuid).ok_or_else(|| {
            OpossumError::Analysis(format!(
                "referenced node with id {target_uuid} not found in graph"
            ))
        })?;

        let target_iso = graph.g[target_idx]
            .positioning()
            .effective_position()
            .copied();

        if let Some(target_iso) = target_iso {
            info!("Target node for reference {node_info} is already placed. Adopting isometry.");
            graph.g[node_idx].set_positioning(NodePositioning::Automatic(Some(target_iso)))?;
            return Ok(());
        }

        position_node(graph, node_idx, incoming_edges, up_direction)?;

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

    position_node(graph, node_idx, incoming_edges, up_direction)?;
    Ok(())
}

fn execute_node_calculation(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_edges: LightResult,
    config: &RayTraceConfig,
) -> OpmResult<LightResult> {
    if graph.g[node_idx].referenced_node_id().is_some() {
        graph.evaluate_node_or_reference(node_idx, |node| {
            AnalysisRayTrace::analyze(&mut **node, incoming_edges, config)
        })
    } else {
        let node = &mut graph.g[node_idx];
        AnalysisRayTrace::calc_node_positions(&mut **node, incoming_edges, config)
    }
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

/// Places one node of the graph on the optical axis and passes the axis on to its successors.
///
/// # Arguments
///
/// * `graph` - The optical graph to operate on.
/// * `node_idx` - Index of the node to place.
/// * `incoming_data` - The optical axis entering the graph through its external input ports.
/// * `up_direction` - Running "up" direction vector, updated by this function.
/// * `config` - The ray tracing configuration of the positioning run.
/// * `light_result` - Collects what leaves the graph through its mapped output ports.
/// * `ray_ends` - Optional collector for unconnected, unmapped ray bundles.
///
/// # Errors
///
/// This function returns an error if the node cannot be placed, if its calculation fails while
/// it has successors, or if its outgoing beams cannot be passed on.
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

    ensure_node_isometry(graph, node_idx, &incoming_edges, up_direction)?;

    let output = execute_node_calculation(graph, node_idx, incoming_edges, config);

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

    graph.collect_group_output_ports(node_id, &outgoing_edges, light_result)?;
    update_outgoing_edges_and_up_direction(
        graph,
        node_idx,
        outgoing_edges,
        up_direction,
        ray_ends,
    )?;

    Ok(())
}
