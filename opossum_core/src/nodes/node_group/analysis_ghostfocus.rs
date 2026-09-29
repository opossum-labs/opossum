#![warn(missing_docs)]
use super::{NodeGroup, optic_graph::InvertGraphGuard};
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus},
    core_optics::{NodeAttrExt, PortType},
    error::{OpmResult, OpossumError},
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

impl AnalysisGhostFocus for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        ray_collection: &mut Vec<Rays>,
        bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let mut current_bouncing_rays = incoming_data;

        let is_inverted = self.inverted();

        // Initialize the RAII guard. This safely handles graph inversion and reverts it
        // when the guard goes out of scope (even on early returns via '?').
        let mut guard = InvertGraphGuard::new(&mut self.graph, is_inverted)?;

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

                let mut outgoing_edges = if let Some(target_uuid) =
                    guard.g[idx].referenced_node_id()
                {
                    // Rename variable to target_is_inverted to avoid shadowing/confusion
                    let target_is_inverted = guard.g[idx].inverted();
                    let target_idx = guard.node_idx_by_uuid(target_uuid).ok_or_else(|| {
                        OpossumError::Analysis(format!(
                            "referenced node with id {target_uuid} not found in graph"
                        ))
                    })?;
                    let target_node = &mut guard.g[target_idx];

                    if target_is_inverted {
                        target_node.set_inverted(true).map_err(|_e| {
                            OpossumError::Analysis(format!(
                                "referenced node {target_node} cannot be inverted"
                            ))
                        })?;
                    }

                    let res = AnalysisGhostFocus::analyze(
                        &mut **target_node,
                        light_result_to_light_rays(incoming_edges)?,
                        config,
                        ray_collection,
                        bounce_lvl,
                    );

                    if target_is_inverted {
                        target_node.set_inverted(false)?;
                    }

                    let node_info = format!("{target_node}");
                    res.map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_info} failed: {e}"))
                    })?
                } else {
                    let node = &mut guard.g[idx];
                    let node_info = format!("{node}");
                    AnalysisGhostFocus::analyze(
                        &mut **node,
                        light_result_to_light_rays(incoming_edges)?,
                        config,
                        ray_collection,
                        bounce_lvl,
                    )
                    .map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_info} failed: {e}"))
                    })?
                };

                filter_ray_limits(&mut outgoing_edges, config);
                current_bouncing_rays.clone_from(&outgoing_edges);

                if guard.is_output_node(node_id)? {
                    let portmap = if guard.is_inverted() {
                        guard.port_map(&PortType::Input).clone()
                    } else {
                        guard.port_map(&PortType::Output).clone()
                    };

                    let assigned_ports = portmap.assigned_ports_for_node(node_id);
                    for port in assigned_ports {
                        if let Some(light_data) = outgoing_edges.get(&port.1) {
                            current_bouncing_rays.insert(port.0, light_data.clone());
                        }
                    }
                }

                let outgoing_edges = light_rays_to_light_result(outgoing_edges);
                for outgoing_edge in outgoing_edges {
                    // Try to push data into the graph. If it fails (no sink/output), we get it back.
                    let leftover_data =
                        guard.set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);

                    // If leftover_data is 'Some', it means the edge does not exist (dead end).
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

        // Manual un-inversion is completely removed here, relying fully on the guard's drop.
        Ok(current_bouncing_rays)
    }
}
