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
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, PortType},
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
            let node_id = self.graph.g[idx].uuid()?;
            let node_info = format!("{}", self.graph.g[idx]);
            if self.graph.is_stale_node(node_id)? {
                warn!("graph contains stale (completely unconnected) node {node_info}. Skipping.");
            } else {
                let incoming_edges = self.graph.take_incoming(node_id, &incoming_data)?;
                let mut outgoing_edges = if let Some(target_uuid) = self.graph.g[idx].referenced_node_id() {
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
                    AnalysisRayTrace::analyze(&mut **node, incoming_edges, config)
                        .map_err(|e| {
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
            let node_id = match self.graph.node_by_idx(idx) {
                Ok(node) => node.uuid()?,
                Err(_) => Uuid::nil(),
            };

            let has_no_input_connections = !self.graph.has_input_connections(node_id)?;

            let already_placed = self.graph.g[idx].isometry().is_some();

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

fn calculate_single_node_position(
    graph: &mut OpticGraph,
    node_idx: NodeIndex,
    incoming_data: &LightResult,
    up_direction: &mut Vector3<f64>,
    config: &RayTraceConfig,
    light_result: &mut LightResult,
) -> OpmResult<()> {
    let node_attr = graph.g[node_idx].node_attr().clone();
    let node_type = node_attr.node_type().to_string();
    let node_isometry = node_attr.isometry();
    let node_info = format!("{}", graph.g[node_idx]);
    let node_id = node_attr.uuid();
    let incoming_edges: LightResult = graph.get_incoming(node_id, incoming_data)?;
    if node_isometry.is_none() {
        if let Some((align_id, distance)) = node_attr.get_align_like_node_at_distance() {
            let align_ref_iso = graph.node(*align_id)?.isometry();
            if let Some(align_ref_iso) = align_ref_iso {
                let align_iso = Isometry::new(
                    Point3::new(Length::zero(), Length::zero(), *distance),
                    radian!(0., 0., 0.),
                )?;
                let new_iso = align_ref_iso.append(&align_iso);
                graph.g[node_idx].set_isometry(new_iso)?;
            } else {
                warn!(
                    "Cannot align node like NodeIdx:{}. Fall back to standard positioning method",
                    node_idx.index()
                );
                graph.set_node_isometry(&incoming_edges, *align_id, *up_direction)?;
            }
        } else {
            graph.set_node_isometry(&incoming_edges, node_id, *up_direction)?;
        }
    } else {
        info!("Node {node_info} has already been placed. Leaving untouched.");
    }
    let output = if let Some(target_uuid) = graph.g[node_idx].referenced_node_id() {
        let is_inverted = graph.g[node_idx].inverted();
        let target_idx = graph.node_idx_by_uuid(target_uuid).ok_or_else(|| {
            OpossumError::Analysis(format!(
                "referenced node with id {target_uuid} not found in graph"
            ))
        })?;
        let target_node = &mut graph.g[target_idx];
        if is_inverted {
            target_node.set_inverted(true).map_err(|_e| {
                OpossumError::Analysis(format!(
                    "referenced node {target_node} cannot be inverted"
                ))
            })?;
        }
        let res = AnalysisRayTrace::calc_node_positions(&mut **target_node, incoming_edges, config);
        if is_inverted {
            target_node.set_inverted(false)?;
        }
        res
    } else {
        let node = &mut graph.g[node_idx];
        AnalysisRayTrace::calc_node_positions(&mut **node, incoming_edges, config)
    };

    let outgoing_edges = match output {
        Ok(edges) => edges,
        Err(e) => {
            // Check, if node has following nodes (=not a pure sink)
            if graph.has_output_connections(node_id)? {
                return Err(OpossumError::Analysis(format!(
                    "calculation of optical axis for node {node_info} failed: {e}"
                )));
            }
            // node has no successors => just issue a warning.
            warn!(
                "Calculation of optical axis for terminal node {node_info} failed: {e}. Ignoring as it has no successors."
            );
            LightResult::default()
        }
    };
    // If node is sink node, rewrite port names according to output mapping
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
    for outgoing_edge in outgoing_edges {
        if let Some(target_uuid) = graph.g[node_idx].referenced_node_id() {
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
            if node_type == "source" || node_type == "source port" {
                *up_direction = node.define_up_direction(&outgoing_edge.1)?;
            } else {
                node.calc_new_up_direction(&outgoing_edge.1, up_direction)?;
            }
        }
        graph.set_outgoing_edge_data(node_idx, &outgoing_edge.0, outgoing_edge.1);
    }
    Ok(())
}
