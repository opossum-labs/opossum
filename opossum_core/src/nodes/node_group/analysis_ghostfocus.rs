#![warn(missing_docs)]
use super::NodeGroup;
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus},
    core_optics::{OpticNode, PortType},
    error::{OpmResult, OpossumError},
    light::{
        LightData, LightRays, Rays,
        light_result::{light_rays_to_light_result, light_result_to_light_rays},
    },
    utils::LockExt,
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
        if self.inverted() {
            self.graph.invert_graph()?;
        }
        if !self.graph.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }
        let sorted = self.graph.topologically_sorted()?;
        for idx in sorted {
            let node_ref = self.graph.node_by_idx(idx)?.optical_ref;
            let node = node_ref.lock_opm()?;
            let node_id = node.node_attr().uuid();
            let node_info = node.to_string();
            drop(node);
            if self.graph.is_stale_node(node_id)? {
                warn!("graph contains stale (completely unconnected) node {node_info}. Skipping.");
            } else {
                let incoming_edges = self.graph.take_incoming(
                    node_id,
                    &light_rays_to_light_result(current_bouncing_rays.clone()),
                )?;
                let mut outgoing_edges = AnalysisGhostFocus::analyze(
                    &mut *node_ref.lock_opm()?,
                    light_result_to_light_rays(incoming_edges)?,
                    config,
                    ray_collection,
                    bounce_lvl,
                )
                .map_err(|e| {
                    OpossumError::Analysis(format!("analysis of node {node_info} failed: {e}"))
                })?;
                filter_ray_limits(&mut outgoing_edges, config);

                current_bouncing_rays.clone_from(&outgoing_edges);

                if self.graph.is_output_node(node_id)? {
                    let portmap = if self.graph.is_inverted() {
                        self.graph.port_map(&PortType::Input).clone()
                    } else {
                        self.graph.port_map(&PortType::Output).clone()
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
                    // Wir versuchen, die Daten in den Graphen zu stecken.
                    // Wenn es nicht klappt (kein Sink/Ausgang), bekommen wir sie zurück.
                    let leftover_data =
                        self.graph
                            .set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);

                    // Wenn leftover_data 'Some' ist, bedeutet das: Die Kante existiert nicht (Sackgasse).
                    // Das entspricht dem alten '!no_sink'.
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
        if self.inverted() {
            self.graph.invert_graph()?;
        } // revert initial inversion (if necessary)
        Ok(current_bouncing_rays)
    }
}
