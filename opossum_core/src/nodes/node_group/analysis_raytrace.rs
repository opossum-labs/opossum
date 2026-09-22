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
    light::{LightData, LightResult},
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
                for outgoing_edge in outgoing_edges {
                    self.graph
                        .set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);
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
                calculate_single_node_position(
                    &mut self.graph,
                    idx,
                    &incoming_data,
                    &mut up_direction,
                    config,
                    &mut light_result,
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
fn update_outgoing_edges_and_up_direction(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    outgoing_edges: LightResult,
    up_direction: &mut Vector3<f64>,
) -> OpmResult<()> {
    let referenced_id = graph.g[node_idx].referenced_node_id();
    let fallback_node_type = graph.g[node_idx].node_attr().node_type().to_string();

    for outgoing_edge in outgoing_edges {
        if let Some(target_uuid) = referenced_id {
            let target_idx = graph.node_idx_by_uuid(target_uuid).ok_or_else(|| {
                OpossumError::Analysis(format!(
                    "referenced node with id {target_uuid} not found in graph"
                ))
            })?;
            let target = &graph.g[target_idx];
            if target.node_type() == "source" || target.node_type() == "source port" {
                *up_direction = target.define_up_direction(&outgoing_edge.1)?;
            } else {
                target.calc_new_up_direction(&outgoing_edge.1, up_direction)?;
            }
        } else {
            let node = &graph.g[node_idx];
            if fallback_node_type == "source" || fallback_node_type == "source port" {
                *up_direction = node.define_up_direction(&outgoing_edge.1)?;
            } else {
                node.calc_new_up_direction(&outgoing_edge.1, up_direction)?;
            }
        }
        graph.set_outgoing_edge_data(node_idx, &outgoing_edge.0, outgoing_edge.1);
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
    update_outgoing_edges_and_up_direction(graph, node_idx, outgoing_edges, up_direction)?;

    Ok(())
}
