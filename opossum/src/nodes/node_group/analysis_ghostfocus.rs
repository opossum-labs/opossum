use super::NodeGroup;
use crate::{
    analyzers::{ghostfocus::AnalysisGhostFocus, GhostFocusConfig},
    error::{OpmResult, OpossumError},
    light_result::{LightDings, LightResult},
    lightdata::LightData,
    optic_ports::PortType,
    rays::Rays,
};
use log::{info, warn};

fn filter_ray_limits(light_result: &mut LightResult, config: &GhostFocusConfig) {
    for lr in light_result {
        if let LightData::Geometric(rays) = lr.1 {
            rays.filter_by_nr_of_bounces(config.max_bounces());
        }
    }
}
fn light_result_to_light_dings_ray(light_result: LightResult) -> OpmResult<LightDings<Rays>> {
    let mut light_dings_rays = LightDings::<Rays>::new();
    for lr in light_result {
        let LightData::Geometric(r) = lr.1 else {
            return Err(OpossumError::Other(
                "no rays data found in LightResult".into(),
            ));
        };
        light_dings_rays.insert(lr.0, r);
    }
    Ok(light_dings_rays)
}
fn light_dings_rays_to_light_result(light_dings: LightDings<Rays>) -> LightResult {
    let mut light_result = LightResult::default();
    for ld in light_dings {
        light_result.insert(ld.0, LightData::Geometric(ld.1));
    }
    light_result
}
// fn merge_light_fields(light_fields: Vec<LightDings<Rays>>) -> LightResult {
//     let mut light_result = LightResult::default();
//     if let Some(light_dings_rays) = light_fields.first() {
//         let ports = light_dings_rays.clone().into_keys();
//         for port in ports {
//             let mut rays = Rays::default();
//             for light_field in &light_fields {
//                 if let Some(new_rays) = light_field.get(&port) {
//                     rays.merge(new_rays);
//                 }
//             }
//             light_result.insert(port, LightData::Geometric(rays));
//         }
//         light_result
//     } else {
//         LightResult::default()
//     }
// }
impl AnalysisGhostFocus for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &GhostFocusConfig,
    ) -> OpmResult<(LightResult, LightResult)> {
        let mut light_field = [light_result_to_light_dings_ray(incoming_data)?];
        for (bounce, _) in light_field.iter().enumerate().take(config.max_bounces()) {
            info!("Analyzing bounce {bounce}...");
            let incoming_data = light_dings_rays_to_light_result(light_field[bounce].clone());
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
                        node.borrow()
                    );
                } else {
                    let incoming_edges = self.graph.get_incoming(idx, &incoming_data);
                    let node_name = format!("{}", node.borrow());
                    //
                    // temporary only
                    //
                    let mut outgoing_edges = AnalysisGhostFocus::analyze(
                        &mut *node.borrow_mut(),
                        incoming_edges,
                        config,
                    )
                    .map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                    })?;
                    //
                    //
                    //
                    filter_ray_limits(&mut outgoing_edges.0, config);
                    filter_ray_limits(&mut outgoing_edges.1, config);
                    // If node is sink node, rewrite port names according to output mapping
                    if self.graph.is_output_node(idx) {
                        let portmap = if self.graph.is_inverted() {
                            self.graph.port_map(&PortType::Input).clone()
                        } else {
                            self.graph.port_map(&PortType::Output).clone()
                        };
                        let assigned_ports = portmap.assigned_ports_for_node(idx);
                        for port in assigned_ports {
                            if let Some(light_data) = outgoing_edges.0.get(&port.1) {
                                light_result.insert(port.0, light_data.clone());
                            }
                        }
                    }
                    for outgoing_edge in outgoing_edges.0 {
                        self.graph
                            .set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);
                    }
                }
            }
            if self.graph.is_inverted() {
                self.graph.invert_graph()?;
            } // revert initial inversion (if necessary)
        }
        Ok((LightResult::default(), LightResult::default()))
    }
}
