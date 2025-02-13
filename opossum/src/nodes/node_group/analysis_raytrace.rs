use log::{info, warn};
use nalgebra::{Point3, Vector3};
use num::Zero;
use uom::si::f64::Length;

use super::NodeGroup;
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
        //self.graph.analyze_raytrace(&incoming_data, config)
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
            let node = g_clone.graph.node_by_idx(idx)?.optical_ref;
            if self.graph.is_stale_node(idx) {
                warn!(
                    "graph contains stale (completely unconnected) node {}. Skipping.",
                    node.lock()
                        .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                );
            } else {
                let incoming_edges = self.graph.get_incoming(idx, &incoming_data);
                let node_name = format!(
                    "{}",
                    node.lock()
                        .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                );
                let mut outgoing_edges = AnalysisRayTrace::analyze(
                    &mut *node
                        .lock()
                        .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?,
                    incoming_edges,
                    config,
                )
                .map_err(|e| {
                    OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                })?;
                filter_ray_limits(&mut outgoing_edges, config);
                // If node is sink node, rewrite port names according to output mapping
                if self.graph.is_output_node(idx) {
                    let portmap = if self.graph.is_inverted() {
                        self.graph.port_map(&PortType::Input).clone()
                    } else {
                        self.graph.port_map(&PortType::Output).clone()
                    };
                    let assigned_ports = portmap.assigned_ports_for_node(idx);
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
    fn calc_node_position(
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
            let node = self.graph.node_by_idx(idx)?.optical_ref;
            let node_type = node
                .lock()
                .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                .node_type();
            let node_attr = node
                .lock()
                .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                .node_attr()
                .clone();
            let incoming_edges: LightResult = self.graph.get_incoming(idx, &incoming_data);
            if node
                .lock()
                .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                .isometry()
                .is_none()
            {
                if incoming_edges.is_empty() {
                    warn!(
                        "{} has no incoming edges",
                        node.lock()
                            .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                    );
                }
                if let Some((node_idx, distance)) = node_attr.get_align_like_node_at_distance() {
                    if let Some(align_ref_iso) = self
                        .graph
                        .node_by_idx(*node_idx)?
                        .optical_ref
                        .lock()
                        .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                        .isometry()
                    {
                        let mut node_borrow_mut = node
                            .lock()
                            .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?;
                        let align_iso = Isometry::new(
                            Point3::new(Length::zero(), Length::zero(), *distance),
                            radian!(0., 0., 0.),
                        )?;
                        let new_iso = align_ref_iso.append(&align_iso);
                        node_borrow_mut.set_isometry(new_iso)?;
                    } else {
                        warn!("Cannot align node like NodeIdx:{}. Fall back to standard positioning method", node_idx.index());
                        self.graph.set_node_isometry(
                            &incoming_edges,
                            &mut node
                                .lock()
                                .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?,
                            &node_type,
                            idx,
                            up_direction,
                        )?;
                    }
                } else {
                    self.graph.set_node_isometry(
                        &incoming_edges,
                        &mut node
                            .lock()
                            .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?,
                        &node_type,
                        idx,
                        up_direction,
                    )?;
                };
            } else {
                info!(
                    "Node {} has already been placed. Leaving untouched.",
                    node.lock()
                        .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                );
            }
            let output = AnalysisRayTrace::calc_node_position(
                &mut *node
                    .lock()
                    .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?,
                incoming_edges,
                config,
            );

            let outgoing_edges = output.map_err(|e| {
                OpossumError::Analysis(format!(
                    "calculation of optical axis for node {} failed: {e}",
                    node.lock().expect("Mutex Lock failed")
                ))
            })?;
            // If node is sink node, rewrite port names according to output mapping
            if self.graph.is_output_node(idx) {
                let portmap = if self.graph.is_inverted() {
                    self.graph.port_map(&PortType::Input).clone()
                } else {
                    self.graph.port_map(&PortType::Output).clone()
                };
                let assigned_ports = portmap.assigned_ports_for_node(idx);
                for port in assigned_ports {
                    if let Some(light_data) = outgoing_edges.get(&port.1) {
                        light_result.insert(port.0, light_data.clone());
                    }
                }
            }
            for outgoing_edge in outgoing_edges {
                if node_type == "source" {
                    up_direction = node
                        .lock()
                        .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                        .define_up_direction(&outgoing_edge.1)?;
                } else {
                    node.lock()
                        .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))?
                        .calc_new_up_direction(&outgoing_edge.1, &mut up_direction)?;
                }

                self.graph
                    .set_outgoing_edge_data(idx, &outgoing_edge.0, &outgoing_edge.1);
            }
        }
        self.reset_data();
        Ok(light_result)
    }
}
