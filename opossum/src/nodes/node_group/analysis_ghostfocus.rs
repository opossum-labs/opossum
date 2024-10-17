use super::NodeGroup;
use crate::{
    analyzers::{ghostfocus::AnalysisGhostFocus, GhostFocusConfig},
    error::{OpmResult, OpossumError},
    light_result::{light_rays_to_light_result, light_result_to_light_rays, LightRays},
    lightdata::LightData,
    optic_node::OpticNode,
    rays::Rays,
};
use log::warn;

fn filter_ray_limits(light_rays: &mut LightRays, config: &GhostFocusConfig) {
    for lr in light_rays {
        lr.1.filter_by_nr_of_bounces(config.max_bounces());
    }
}
impl AnalysisGhostFocus for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        ray_collection: &mut Vec<Rays>,
    ) -> OpmResult<LightRays> {
        let mut current_bouncing_rays = incoming_data;

        if self.inverted() {
            self.graph.invert_graph()?;
        }

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
                    &light_rays_to_light_result(current_bouncing_rays.clone()),
                );
                let node_name = format!("{}", node.borrow());

                let mut outgoing_edges = AnalysisGhostFocus::analyze(
                    &mut *node.borrow_mut(),
                    light_result_to_light_rays(incoming_edges)?,
                    config,
                    ray_collection,
                )
                .map_err(|e| {
                    OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                })?;
                filter_ray_limits(&mut outgoing_edges, config);
                current_bouncing_rays.clone_from(&outgoing_edges);
                let outgoing_edges = light_rays_to_light_result(outgoing_edges);

                for outgoing_edge in outgoing_edges {
                    println!(
                        "{}",
                        self.graph.node_by_idx(idx)?.optical_ref.borrow().name()
                    );
                    let no_sink =
                        self.graph
                            .set_outgoing_edge_data(idx, &outgoing_edge.0, &outgoing_edge.1);

                    if !no_sink {
                        if let LightData::Geometric(rays) = outgoing_edge.1 {
                            ray_collection.push(rays);
                        }
                    }
                }
            }
        }
        if self.inverted() {
            self.graph.invert_graph()?;
            self.set_inverted(false)?;
        } // revert initial inversion (if necessary)
        Ok(current_bouncing_rays)
    }
}
