use super::NodeGroup;
use crate::{
    analyzers::{ghostfocus::AnalysisGhostFocus, GhostFocusConfig},
    error::{OpmResult, OpossumError},
    light_result::{light_rays_to_light_result, light_result_to_light_rays, LightRays},
    lightdata::LightData,
    optic_node::OpticNode,
    optic_ports::PortType, rays::Rays,
};
use log::{info, warn};

fn filter_ray_limits(light_rays: &mut LightRays, config: &GhostFocusConfig) {
    for lr in light_rays {
        lr.1.filter_by_nr_of_bounces(config.max_bounces() + 1);
    }
}
impl AnalysisGhostFocus for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
    ) -> OpmResult<LightRays> {
        let mut all_propagting_rays=Rays::default();
        let mut current_bouncing_rays = incoming_data;
        let mut group_inversion = self.inverted();
        for pass in 0..=config.max_bounces() {
            let direction = if group_inversion {
                "backward"
            } else {
                "forward"
            };
            info!("Analyzing pass {pass} ({direction}) ...");
            if group_inversion {
                self.graph.invert_graph()?;
            }
            let current_bounce_data = current_bouncing_rays.clone();
            let g_clone = self.clone();
            if !self.graph.is_single_tree() {
                warn!("group contains unconnected sub-trees. Analysis might not be complete.");
            }
            let sorted = self.graph.topologically_sorted()?;
            for idx in sorted {
                let node = g_clone.graph.node_by_idx(idx)?.optical_ref;
                if self.graph.is_stale_node(idx) {
                    warn!(
                        "graph contains stale (completely unconnected) node {}. Skipping.",
                        node.borrow()
                    );
                } else {
                    let incoming_edges = self.graph.get_incoming(
                        idx,
                        &light_rays_to_light_result(current_bounce_data.clone()),
                    );
                    let node_name = format!("{}", node.borrow());

                    let mut outgoing_edges = AnalysisGhostFocus::analyze(
                        &mut *node.borrow_mut(),
                        light_result_to_light_rays(incoming_edges)?,
                        config,
                    )
                    .map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                    })?;
                    filter_ray_limits(&mut outgoing_edges, config);
                    for rays in &outgoing_edges {
                        all_propagting_rays.merge(rays.1);
                    }
                    current_bouncing_rays.clone_from(&outgoing_edges);
                    let outgoing_edges = light_rays_to_light_result(outgoing_edges);

                    // If node is sink node, rewrite port names according to output mapping
                    if self.graph.is_output_node(idx) {
                        let portmap = if self.graph.is_inverted() {
                            self.graph.port_map(&PortType::Input).clone()
                        } else {
                            self.graph.port_map(&PortType::Output).clone()
                        };
                        let assigned_ports = portmap.assigned_ports_for_node(idx);
                        for port in assigned_ports {
                            if let Some(LightData::Geometric(_rays)) = outgoing_edges.get(&port.1) {
                                // light_result.insert(port.0, rays.clone());
                            }
                        }
                    }
                    for outgoing_edge in outgoing_edges {
                        self.graph
                            .set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);
                    }
                }
            }
            if group_inversion {
                self.graph.invert_graph()?;
            } // revert initial inversion (if necessary)
            group_inversion = !group_inversion;
        }
        self.accumulated_rays=all_propagting_rays;
        Ok(current_bouncing_rays)
    }
}
