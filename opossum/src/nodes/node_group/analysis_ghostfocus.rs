use super::NodeGroup;
use crate::{
    analyzers::{ghostfocus::AnalysisGhostFocus, GhostFocusConfig},
    error::{OpmResult, OpossumError},
    light_result::{
        light_bouncing_rays_to_light_rays, light_bouncing_rays_to_light_result,
        light_rays_to_light_bouncing_rays, light_rays_to_light_result, light_result_to_light_rays,
        LightBouncingRays, LightResult,
    },
    lightdata::LightData,
    optic_ports::PortType,
};
use log::{info, warn};

fn filter_ray_limits(light_result: &mut LightResult, config: &GhostFocusConfig) {
    for lr in light_result {
        if let LightData::Geometric(rays) = lr.1 {
            rays.filter_by_nr_of_bounces(config.max_bounces());
        }
    }
}
impl AnalysisGhostFocus for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightBouncingRays,
        config: &GhostFocusConfig,
    ) -> OpmResult<LightBouncingRays> {
        for bounce in 0..config.max_bounces() {
            info!("Analyzing bounce {bounce}...");
            let incoming_data = light_bouncing_rays_to_light_rays(incoming_data.clone(), bounce)?;
            if self.graph.is_inverted() {
                self.graph.invert_graph()?;
            }
            let g_clone = self.clone();
            if !self.graph.is_single_tree() {
                warn!("group contains unconnected sub-trees. Analysis might not be complete.");
            }
            let sorted = self.graph.topologically_sorted()?;
            //let mut light_result = incoming_data.clone();
            for idx in sorted {
                let node = g_clone.graph.node_by_idx(idx)?.optical_ref;
                if self.graph.is_stale_node(idx) {
                    warn!(
                        "graph contains stale (completely unconnected) node {}. Skipping.",
                        node.borrow()
                    );
                } else {
                    let incoming_edges = self
                        .graph
                        .get_incoming(idx, &light_rays_to_light_result(incoming_data.clone()));
                    let node_name = format!("{}", node.borrow());
                    //
                    // temporary only
                    //
                    let outgoing_edges = AnalysisGhostFocus::analyze(
                        &mut *node.borrow_mut(),
                        light_rays_to_light_bouncing_rays(light_result_to_light_rays(
                            incoming_edges,
                        )?),
                        config,
                    )
                    .map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                    })?;
                    let mut outgoing_edges = light_bouncing_rays_to_light_result(outgoing_edges)?;
                    //
                    //
                    //
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
            if self.graph.is_inverted() {
                self.graph.invert_graph()?;
            } // revert initial inversion (if necessary)
        }
        Ok(LightBouncingRays::default())
    }
}
