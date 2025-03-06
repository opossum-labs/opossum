use log::{info, warn};
use nalgebra::{Point3, Vector3};
use num::Zero;
use petgraph::graph::NodeIndex;
use uom::si::f64::Length;

use super::{NodeGroup, OpticGraph};
use crate::{
    analyzers::{raytrace::AnalysisRayTrace, RayTraceConfig},
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    lightdata::LightData,
    optic_node::OpticNode,
    optic_ports::PortType,
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
        let g_clone = self.clone();
        if !self.graph.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }
        let sorted = self.graph.topologically_sorted()?;
        let mut light_result = incoming_data.clone();
        for idx in sorted {
            let node_ref = g_clone.graph.node_by_idx(idx)?.optical_ref;
            let node = node_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
            let node_info = node.to_string();
            let node_id = *node.node_attr().uuid();
            drop(node);
            if self.graph.is_stale_node(&node_id) {
                warn!(
                    "graph contains stale (completely unconnected) node {}. Skipping.",
                    node_info
                );
            } else {
                let incoming_edges = self.graph.get_incoming(&node_id, &incoming_data);
                let mut outgoing_edges = AnalysisRayTrace::analyze(
                    &mut *node_ref
                        .lock()
                        .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?,
                    incoming_edges,
                    config,
                )
                .map_err(|e| {
                    OpossumError::Analysis(format!("analysis of node {node_info} failed: {e}"))
                })?;
                filter_ray_limits(&mut outgoing_edges, config);
                // If node is sink node, rewrite port names according to output mapping
                if self.graph.is_output_node(idx) {
                    let portmap = if self.graph.is_inverted() {
                        self.graph.port_map(&PortType::Input).clone()
                    } else {
                        self.graph.port_map(&PortType::Output).clone()
                    };
                    let assigned_ports = portmap.assigned_ports_for_node(&node_id);
                    for port in assigned_ports {
                        if let Some(light_data) = outgoing_edges.get(&port.1) {
                            light_result.insert(port.0, light_data.clone());
                        }
                    }
                }
                for outgoing_edge in outgoing_edges {
                    self.graph
                        .set_outgoing_edge_data(idx, &outgoing_edge.0, &outgoing_edge.1);
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
        // set stored distances from predecessors
        self.graph
            .set_external_distances(self.input_port_distances.clone());

        let sorted = self.graph.topologically_sorted()?;
        let mut light_result = LightResult::default();
        let mut up_direction = Vector3::<f64>::y();
        for idx in sorted {
            let node_name = self
                .graph
                .node_by_idx(idx)
                .unwrap()
                .optical_ref
                .lock()
                .unwrap()
                .node_attr()
                .name();
            println!("node: {}", node_name);
            calculate_single_node_position(
                &mut self.graph,
                idx,
                &incoming_data,
                &mut up_direction,
                config,
                &mut light_result,
            )?;
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
    let node_ref = graph.node_by_idx(node_idx)?.optical_ref;
    let node = node_ref
        .lock()
        .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
    let node_attr = node.node_attr().clone();
    let node_type = node_attr.node_type();
    let node_isometry = node_attr.isometry();
    let node_info = node.to_string();
    let node_id = node_attr.uuid();
    drop(node);
    let incoming_edges: LightResult = graph.get_incoming(node_id, incoming_data);
    if node_isometry.is_none() {
        if incoming_edges.is_empty() {
            warn!("{node_info} has no incoming edges");
        }
        if let Some((node_id, distance)) = node_attr.get_align_like_node_at_distance() {
            let align_ref_iso = graph
                .node_by_uuid(node_id)?
                .optical_ref
                .lock()
                .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?
                .isometry();
            if let Some(align_ref_iso) = align_ref_iso {
                let align_iso = Isometry::new(
                    Point3::new(Length::zero(), Length::zero(), *distance),
                    radian!(0., 0., 0.),
                )?;
                let new_iso = align_ref_iso.append(&align_iso);
                let mut node = node_ref
                    .lock()
                    .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
                node.set_isometry(new_iso)?;
                drop(node);
            } else {
                warn!(
                    "Cannot align node like NodeIdx:{}. Fall back to standard positioning method",
                    node_idx.index()
                );
                graph.set_node_isometry(&incoming_edges, node_id, *up_direction)?;
            }
        } else {
            graph.set_node_isometry(&incoming_edges, node_id, *up_direction)?;
        };
    } else {
        info!("Node {node_info} has already been placed. Leaving untouched.");
    }
    let output = AnalysisRayTrace::calc_node_positions(
        &mut *node_ref
            .lock()
            .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?,
        incoming_edges,
        config,
    );
    let outgoing_edges = output.map_err(|e| {
        OpossumError::Analysis(format!(
            "calculation of optical axis for node {node_info} failed: {e}"
        ))
    })?;
    // If node is sink node, rewrite port names according to output mapping
    if graph.is_output_node(node_idx) {
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
        let node = node_ref
            .lock()
            .map_err(|_| OpossumError::Other("Mutex lock failed".to_string()))?;
        if node_type == "source" {
            *up_direction = node.define_up_direction(&outgoing_edge.1)?;
        } else {
            node.calc_new_up_direction(&outgoing_edge.1, up_direction)?;
        }
        drop(node);
        graph.set_outgoing_edge_data(node_idx, &outgoing_edge.0, &outgoing_edge.1);
    }
    Ok(())
}
