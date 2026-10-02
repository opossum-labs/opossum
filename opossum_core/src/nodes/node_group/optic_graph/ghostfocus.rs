#![warn(missing_docs)]

use super::{InvertGraphGuard, OpticGraph};
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus},
    error::OpmResult,
    light::{
        LightData, LightRays, Rays,
        light_result::{light_rays_to_light_result, light_result_to_light_rays},
    },
};
use log::warn;

fn filter_ray_limits(light_rays: &mut LightRays, config: &GhostFocusConfig) {
    for lr in light_rays {
        for rays in lr.1 {
            rays.filter_by_nr_of_bounces(config.max_bounces());
        }
    }
}

impl OpticGraph {
    /// Performs a ghost focus analysis on this optical graph.
    ///
    /// # Errors
    ///
    /// This function returns an error if various underlying functions fail.
    pub fn analyze_ghostfocus(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        ray_collection: &mut Vec<Rays>,
        bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let mut current_bouncing_rays = incoming_data;
        let is_inverted = self.is_inverted();

        let mut guard = InvertGraphGuard::new(self, is_inverted)?;

        if !guard.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }

        let sorted = guard.topologically_sorted()?;
        for idx in sorted {
            let node_id = guard.g[idx].uuid();
            let node_info = format!("{}", guard.g[idx]);

            if guard.is_stale_node(node_id)? {
                warn!("graph contains stale (completely unconnected) node {node_info}. Skipping.");
            } else {
                let incoming_edges = guard.take_incoming(
                    node_id,
                    &light_rays_to_light_result(current_bouncing_rays.clone()),
                )?;

                // Evaluate node or reference proxy using shared helper
                let mut outgoing_edges = guard.evaluate_node_or_reference(idx, |node| {
                    AnalysisGhostFocus::analyze(
                        &mut **node,
                        light_result_to_light_rays(incoming_edges)?,
                        config,
                        ray_collection,
                        bounce_lvl,
                    )
                })?;

                filter_ray_limits(&mut outgoing_edges, config);
                current_bouncing_rays.clone_from(&outgoing_edges);

                // Map outgoing ports to external group ports
                guard.collect_group_output_ports(
                    node_id,
                    &outgoing_edges,
                    &mut current_bouncing_rays,
                )?;

                let outgoing_edges = light_rays_to_light_result(outgoing_edges);
                for outgoing_edge in outgoing_edges {
                    let leftover_data =
                        guard.set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);

                    if let Some(data) = leftover_data
                        && let LightData::GhostFocus(rays) = data
                    {
                        for r in rays {
                            ray_collection.push(r);
                        }
                    }
                }
            }
        }

        Ok(current_bouncing_rays)
    }
}
