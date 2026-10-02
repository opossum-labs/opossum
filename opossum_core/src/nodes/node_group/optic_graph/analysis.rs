use log::{error, warn};
use nalgebra::Vector3;
use petgraph::{Direction, algo::toposort, graph::NodeIndex, visit::EdgeRef};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};
use uom::si::{
    angle::degree,
    f64::Length,
    length::{meter, millimeter},
};
use uuid::Uuid;

use crate::{
    analyzers::energy::{AnalysisEnergy, EnergyConfig},
    core_optics::{NodeAttrExt, OpticRef, PortType, node_attr::NodePositioning},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult},
    nanometer,
    nodes::NodeGroup,
    radian,
    utils::geom_transformation::{AxisMismatch, Isometry},
};

use super::OpticGraph;

/// A guard that safely manages the inversion state of an `OpticGraph`.
pub struct InvertGraphGuard<'a> {
    graph: &'a mut OpticGraph,
    needs_revert: bool,
}

impl<'a> InvertGraphGuard<'a> {
    /// Creates a new guard, inverting the graph if `invert` is true.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::OpticGroup`] if inverting the graph fails
    /// (e.g. if the graph contains a non-invertible node).
    pub(crate) fn new(graph: &'a mut OpticGraph, invert: bool) -> OpmResult<Self> {
        if invert {
            graph.invert_graph()?;
        }
        Ok(Self {
            graph,
            needs_revert: invert,
        })
    }
}

impl Drop for InvertGraphGuard<'_> {
    fn drop(&mut self) {
        if self.needs_revert
            && let Err(e) = self.graph.invert_graph()
        {
            error!("Critical: Failed to revert graph inversion during cleanup: {e}");
        }
    }
}

impl Deref for InvertGraphGuard<'_> {
    type Target = OpticGraph;

    fn deref(&self) -> &Self::Target {
        self.graph
    }
}

impl DerefMut for InvertGraphGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.graph
    }
}

/// Logs an actionable warning that the axis reaching `port` is inconsistent with the node's placement.
///
/// Reports the angular deviation, how far the beam arrives off along and across the axis (with the
/// connection length that would fix the axial part), and the pose the beam at `port` would need at
/// its predecessor to be consistent. For a directly connected source that pose is the absolute
/// position and direction to give that source. The node keeps the placement it already has.
fn warn_inconsistent_axis(
    node_name: &str,
    primary_port: Option<&str>,
    port: &str,
    distance: Length,
    expected_entrance: &Isometry,
    mismatch: &AxisMismatch,
) {
    let placed_from = primary_port.map_or_else(
        || "its fixed position".to_owned(),
        |p| format!("input port '{p}'"),
    );
    let direction = expected_entrance.transform_vector_f64(&Vector3::z());
    let origin = expected_entrance.translation();
    let corrected = distance - mismatch.axial;
    // Point on the predecessor from which the axis must start to reach the node consistently.
    let d = distance.get::<meter>();
    let start_x = d.mul_add(-direction.x, origin.x.get::<meter>()) * 1.0e3;
    let start_y = d.mul_add(-direction.y, origin.y.get::<meter>()) * 1.0e3;
    let start_z = d.mul_add(-direction.z, origin.z.get::<meter>()) * 1.0e3;
    warn!(
        "Node '{node_name}' is placed from {placed_from}, but the beam reaching input port '{port}' is not consistent with that placement:"
    );
    warn!(
        "  - its direction differs by {:.4}°",
        mismatch.angle.get::<degree>()
    );
    warn!(
        "  - it arrives {:.3} mm off along the axis (connection length should be {:.3} mm instead of {:.3} mm)",
        mismatch.axial.get::<millimeter>(),
        corrected.get::<millimeter>(),
        distance.get::<millimeter>()
    );
    warn!(
        "  - it misses the axis sideways by {:.3} mm",
        mismatch.lateral.get::<millimeter>()
    );
    warn!(
        "  - to be consistent, the beam at '{port}' must start at ({start_x:.3}, {start_y:.3}, {start_z:.3}) mm and travel along ({:.4}, {:.4}, {:.4})",
        direction.x, direction.y, direction.z
    );
    warn!("  keeping the placement from {placed_from}.");
}

impl OpticGraph {
    /// Evaluates an analysis action on the node at `idx`.
    ///
    /// If the node is a reference proxy (`referenced_node_id` is present), this method
    /// looks up the target node, handles temporary inversion if required, executes `action`,
    /// reverts the inversion, and attaches proper error context.
    /// If the node is a standard node, `action` is executed directly.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Analysis`] if:
    /// - A referenced node with the target UUID is not found in the graph.
    /// - A referenced node cannot be inverted or reverted.
    /// - The analysis `action` closure fails on the target node.
    pub fn evaluate_node_or_reference<F, R>(&mut self, idx: NodeIndex, action: F) -> OpmResult<R>
    where
        F: FnOnce(&mut OpticRef) -> OpmResult<R>,
    {
        if let Some(target_uuid) = self.g[idx].referenced_node_id() {
            let target_is_inverted = self.g[idx].inverted();
            let target_idx = self.node_idx_by_uuid(target_uuid).ok_or_else(|| {
                OpossumError::Analysis(format!(
                    "referenced node with id {target_uuid} not found in graph"
                ))
            })?;

            let target_node = &mut self.g[target_idx];
            if target_is_inverted {
                target_node.set_inverted(true).map_err(|_e| {
                    OpossumError::Analysis(format!(
                        "referenced node {target_node} cannot be inverted"
                    ))
                })?;
            }

            let res = action(target_node);

            if target_is_inverted {
                target_node.set_inverted(false)?;
            }

            let node_name = format!("{target_node}");
            res.map_err(|e| {
                OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
            })
        } else {
            let node = &mut self.g[idx];
            let node_name = format!("{node}");
            action(node).map_err(|e| {
                OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
            })
        }
    }

    /// If `node_id` is an output node of the group, maps its outgoing data to the
    /// external group port names and inserts them into `group_output`.
    ///
    /// # Errors
    ///
    /// Returns an error if determining whether the node is an output node
    /// fails (e.g. if the node with the given `node_id` does not exist).
    pub fn collect_group_output_ports<V: Clone>(
        &self,
        node_id: Uuid,
        outgoing_edges: &HashMap<String, V>,
        group_output: &mut HashMap<String, V>,
    ) -> OpmResult<()> {
        if self.is_output_node(node_id)? {
            let portmap = if self.is_inverted() {
                &self.input_port_map
            } else {
                &self.output_port_map
            };

            for (ext_port, int_port) in portmap.assigned_ports_for_node(node_id) {
                if let Some(data) = outgoing_edges.get(&int_port) {
                    group_output.insert(ext_port, data.clone());
                }
            }
        }
        Ok(())
    }

    /// Returns the incoming data of a node in this [`OpticGraph`].
    ///
    /// This function returns the incoming data of a node with the given [`Uuid`]. If the node is an external node, the
    /// incoming data is mapped to the internal node names.
    ///
    /// # Errors
    ///
    /// Returns an error if the given `node_id` does not exist in the graph.
    pub fn get_incoming(
        &self,
        node_id: Uuid,
        incoming_data: &LightResult,
    ) -> OpmResult<LightResult> {
        if self.is_incoming_node(node_id)? {
            let portmap = if self.is_inverted() {
                &self.output_port_map
            } else {
                &self.input_port_map
            };

            let mut mapped_light_result = LightResult::default();

            for incoming in incoming_data {
                if let Some(mapping) = portmap.get(incoming.0)
                    && node_id == mapping.0
                {
                    mapped_light_result.insert(mapping.1.clone(), incoming.1.clone());
                }
            }

            for edge in self.incoming_edges(node_id) {
                mapped_light_result.insert(edge.0.clone(), edge.1.clone());
            }

            Ok(mapped_light_result)
        } else {
            Ok(self.incoming_edges(node_id))
        }
    }

    /// Moves out the incoming data of a node in this [`OpticGraph`].
    ///
    /// This function returns the incoming data of a node with the given [`Uuid`]. If the node is an external node, the
    /// incoming data is mapped to the internal node names. This function is similar to `get_incoming` but it has move semantics.
    ///
    /// # Errors
    ///
    /// Returns an error if the given `node_id` does not exist in the graph.
    pub fn take_incoming(
        &mut self,
        node_id: Uuid,
        incoming_data: &LightResult,
    ) -> OpmResult<LightResult> {
        if self.is_incoming_node(node_id)? {
            let portmap = if self.is_inverted() {
                &self.output_port_map
            } else {
                &self.input_port_map
            };

            let mut mapped_light_result = LightResult::default();

            for incoming in incoming_data {
                if let Some(mapping) = portmap.get(incoming.0)
                    && node_id == mapping.0
                {
                    mapped_light_result.insert(mapping.1.clone(), incoming.1.clone());
                }
            }

            let internal_data = self.take_incoming_edges(node_id);
            for (port, data) in internal_data {
                mapped_light_result.insert(port, data);
            }

            Ok(mapped_light_result)
        } else {
            Ok(self.take_incoming_edges(node_id))
        }
    }

    fn take_incoming_edges(&mut self, node_id: Uuid) -> LightResult {
        let node_idx = self.node_idx_by_uuid(node_id).unwrap();
        let mut edges_data = LightResult::new();
        let mut edge_indices = Vec::new();

        for edge in self.g.edges_directed(node_idx, Direction::Incoming) {
            edge_indices.push(edge.id());
        }

        for edge_idx in edge_indices {
            if let Some(edge_weight) = self.g.edge_weight_mut(edge_idx)
                && edge_weight.data().is_some()
                && let Some(data) = edge_weight.data_mut().take()
            {
                edges_data.insert(edge_weight.target_port().to_owned(), data);
            }
        }
        edges_data
    }

    /// Clear the [`LightData`] stored in the edges of this [`OpticGraph`]. Useful for back-
    /// and forth-propagation in ghost focus analysis.
    pub fn clear_edges(&mut self) {
        for edge in self.g.edge_weights_mut() {
            edge.set_data(None);
        }
    }

    /// Returns the topologically sorted indices of this [`OpticGraph`].
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Analysis`] if topological sorting fails because
    /// the graph contains directed cycles.
    pub fn topologically_sorted(&self) -> OpmResult<Vec<NodeIndex>> {
        toposort(&self.g, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))
    }

    /// Performs an energy flow analysis of this graph.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Inverting the graph fails.
    /// - Topological sorting of the graph fails due to cyclic connections.
    /// - Resolving a node index to its UUID fails.
    /// - Checking connectivity status (`is_stale_node`, `is_incoming_node`, or `is_output_node`) fails.
    /// - Retrieving incoming light data for a node fails.
    /// - Analysis of any individual or referenced node fails.
    pub fn analyze_energy(
        &mut self,
        incoming_data: &LightResult,
        config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        let is_inverted = self.is_inverted();
        let mut guard = InvertGraphGuard::new(self, is_inverted)?;

        if !guard.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }

        let sorted = guard.topologically_sorted()?;
        let mut light_result = LightResult::default();

        for idx in sorted {
            let node_id = guard.node_by_idx(idx)?.uuid();

            if guard.is_stale_node(node_id)? {
                let node_name = format!("{}", guard.g[idx]);
                warn!("graph contains stale (completely unconnected) node {node_name}. Skipping.");
            } else {
                let incoming_edges = guard.take_incoming(node_id, incoming_data)?;

                // Using the unified helper for node / reference evaluation
                let outgoing_edges = guard.evaluate_node_or_reference(idx, |node| {
                    AnalysisEnergy::analyze(&mut **node, incoming_edges, config)
                })?;

                // Using the unified helper for group port mapping
                guard.collect_group_output_ports(node_id, &outgoing_edges, &mut light_result)?;

                for outgoing_edge in outgoing_edges {
                    guard.set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);
                }
            }
        }

        Ok(light_result)
    }

    /// Returns the (optical) distance to a connected predecessor node.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Analysis`] if:
    /// - The target port is mapped externally, but its distance is not present in the external distances map.
    /// - The node with the given `node_id` does not exist in the graph.
    /// - No connecting edge exists from any predecessor node.
    /// - No connecting predecessor edge targets the specified `port_name`.
    pub fn distance_from_predecessor(&self, node_id: Uuid, port_name: &str) -> OpmResult<Length> {
        let portmap = if self.is_inverted() {
            &self.output_port_map
        } else {
            &self.input_port_map
        };

        if let Some(external_port_name) = portmap.external_port_name(node_id, port_name) {
            self.external_distances().get(&external_port_name).map_or_else(
                || {
                    Err(OpossumError::Analysis(format!(
                        "did not find distance from predecessor to target port '{port_name}' because it's not in the list of external distances"
                    )))
                },
                |length| Ok(*length),
            )
        } else {
            let idx = self.node_idx_by_uuid(node_id).ok_or_else(|| {
                OpossumError::Analysis(format!("node with id {node_id} not found in graph"))
            })?;

            let neighbors = self
                .g
                .neighbors_directed(idx, petgraph::Direction::Incoming);

            let mut length = None;
            for neighbor in neighbors {
                let Some(connecting_edge_ref) = self.g.edges_connecting(neighbor, idx).next()
                else {
                    return Err(OpossumError::Analysis(
                        "could not find connecting edge from predecessor".into(),
                    ));
                };

                let connecting_edge = connecting_edge_ref.weight();
                if connecting_edge.target_port() == port_name {
                    length = Some(connecting_edge.distance());
                }
            }
            length.map_or_else(
                || {
                    Err(OpossumError::Analysis(
                        "did not find distance from predecessor to target port".into(),
                    ))
                },
                |length| Ok(*length),
            )
        }
    }

    /// Sets the node isometry of this [`OpticGraph`].
    ///
    /// The node is placed from the first connected input port (in the ports' deterministic declared
    /// order) that carries axis data. Every further connected port is only checked against that
    /// placement: an inconsistent one is reported as a warning (with a suggested fix), not an error,
    /// and the placement is kept.
    ///
    /// # Errors
    ///
    /// Returns an [`OpossumError::Analysis`] if:
    /// - The node with the given `node_id` is not found in the graph.
    /// - Determining the distance from the predecessor node fails.
    /// - The incoming light data is not of type [`LightData::Geometric`].
    /// - The incoming ray bundle contains no rays.
    /// - Ray propagation across the distance fails.
    pub fn set_node_isometry(
        &mut self,
        incoming_edges: &LightResult,
        node_id: Uuid,
        up_direction: Vector3<f64>,
    ) -> OpmResult<()> {
        let idx = self.node_idx_by_uuid(node_id).ok_or_else(|| {
            OpossumError::Analysis(format!("node with id {node_id} not found in graph"))
        })?;
        // Tolerances that only absorb floating-point noise; any larger deviation is a real
        // inconsistency between two inputs and is reported.
        let angle_tolerance = radian!(1.0e-9);
        let position_tolerance = nanometer!(1.0);
        // The port that placed the node, for the "keeping the placement from ..." hint. `None` means
        // the node was already placed before this call (e.g. an absolute position).
        let mut primary_port: Option<String> = None;
        // Iterate the input ports in their (deterministic) declared order, not in the incoming
        // data's hash order, so the node is always placed from the same port.
        let input_ports = self.g[idx].ports().names(&PortType::Input);
        for port_name in input_ports {
            let Some(incoming) = incoming_edges.get(&port_name) else {
                continue;
            };
            let distance_from_predecessor = self.distance_from_predecessor(node_id, &port_name)?;
            let node = &mut self.g[idx];

            if let Some(group) = node.as_any_mut().downcast_mut::<NodeGroup>() {
                group.add_input_port_distance(&port_name, distance_from_predecessor);
            }
            let LightData::Geometric(rays) = incoming else {
                return Err(OpossumError::Analysis(
                    "expected LightData::Geometric at input port".into(),
                ));
            };
            let Some(ray) = rays.into_iter().next() else {
                return Err(OpossumError::Analysis(
                    "no rays in this ray bundle. cannot position nodes".into(),
                ));
            };
            let mut ray = ray.to_owned();
            ray.propagate(distance_from_predecessor)?;
            let world_frame = ray.to_isometry(up_direction);
            let entrance_frame = self.axis_entrance_frame_of(node_id, &port_name);
            if let Some(placed_iso) = self.g[idx].positioning().effective_position().copied() {
                let expected_entrance = placed_iso.append(&entrance_frame);
                if let Some(mismatch) = expected_entrance.axis_mismatch(
                    &world_frame,
                    angle_tolerance,
                    position_tolerance,
                ) {
                    warn_inconsistent_axis(
                        self.g[idx].name(),
                        primary_port.as_deref(),
                        &port_name,
                        distance_from_predecessor,
                        &expected_entrance,
                        &mismatch,
                    );
                }
            } else {
                let node_iso = world_frame.append(&Isometry::new_from_transform(
                    entrance_frame.get_inv_transform(),
                ));
                self.g[idx].set_positioning(NodePositioning::Automatic(Some(node_iso)))?;
                primary_port = Some(port_name.clone());
            }
        }
        Ok(())
    }
    /// Returns the axis entrance frame of input port `port_name` of the node `node_id`.
    ///
    /// This is the local frame from which the optical axis enters the port (see
    /// [`AnalysisRayTrace::axis_entrance_frame`]). It is the identity for ordinary single-input
    /// nodes. For a reference proxy the referenced target answers, since the proxy has no geometry
    /// of its own. Returns the identity if the node cannot be found.
    fn axis_entrance_frame_of(&self, node_id: Uuid, port_name: &str) -> Isometry {
        let Some(idx) = self.node_idx_by_uuid(node_id) else {
            return Isometry::identity();
        };
        let node = &self.g[idx];
        if let Some(target_uuid) = node.referenced_node_id()
            && let Some(target_idx) = self.node_idx_by_uuid(target_uuid)
        {
            return self.g[target_idx].axis_entrance_frame(port_name);
        }
        node.axis_entrance_frame(port_name)
    }
    /// Sets the outgoing edge data of this [`OpticGraph`].
    /// Returns `None` if data has been assigned to an edge, or `Some(data)` if no matching edge was found.
    pub fn set_outgoing_edge_data(
        &mut self,
        idx: NodeIndex,
        port: &str,
        data: LightData,
    ) -> Option<LightData> {
        let edges = self.g.edges_directed(idx, Direction::Outgoing);
        let mut target_edge_idx = None;

        for edge in edges {
            if edge.weight().src_port() == port {
                target_edge_idx = Some(edge.id());
                break;
            }
        }

        if let Some(edge_idx) = target_edge_idx {
            if let Some(light) = self.g.edge_weight_mut(edge_idx) {
                light.set_data(Some(data));
            }
            None
        } else {
            Some(data)
        }
    }

    fn incoming_edges(&self, node_id: Uuid) -> LightResult {
        let node_idx = self.node_idx_by_uuid(node_id).unwrap();
        let edges = self.g.edges_directed(node_idx, Direction::Incoming);

        edges
            .into_iter()
            .filter(|e| e.weight().data().is_some())
            .map(|e| {
                (
                    e.weight().target_port().to_owned(),
                    e.weight().data().cloned().unwrap(),
                )
            })
            .collect::<LightResult>()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        core_optics::OpticNode,
        core_optics::PortType,
        light::spectrum_helper::create_he_ne_spec,
        nodes::{BeamSplitter, Dummy, SourcePort, SplittingConfigBuilder},
        utils::{geom_transformation::Isometry, test_helper::helper::check_logs},
    };
    use approx::assert_abs_diff_eq;
    use num_traits::Zero;

    #[test]
    fn analyze_empty() -> OpmResult<()> {
        let mut node = OpticGraph::default();
        let output = node.analyze_energy(&LightResult::default(), &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }

    #[test]
    fn analyze_subtree_warning() -> OpmResult<()> {
        let mut graph = OpticGraph::default();

        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let d1 = graph.add_node(dummy)?;

        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let d2 = graph.add_node(dummy)?;

        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let d3 = graph.add_node(dummy)?;

        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let d4 = graph.add_node(dummy)?;

        graph.connect_nodes(d1, "output_1", d2, "input_1", Length::zero())?;
        graph.connect_nodes(d3, "output_1", d4, "input_1", Length::zero())?;

        graph.map_port(d1, &PortType::Input, "input_1", "input_1")?;

        let input = LightResult::default();
        testing_logger::setup();
        graph.analyze_energy(&input, &EnergyConfig::default())?;

        check_logs(
            log::Level::Warn,
            vec!["group contains unconnected sub-trees. Analysis might not be complete."],
        );
        Ok(())
    }

    #[test]
    fn analyze_stale_node() -> OpmResult<()> {
        let mut graph = OpticGraph::default();

        let mut dummy = Dummy::default();
        dummy.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let d1 = graph.add_node(dummy)?;
        let _ = graph.add_node(Dummy::new("stale node"))?;

        graph.map_port(d1, &PortType::Input, "input_1", "input_1")?;

        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Fourier);
        testing_logger::setup();

        assert!(
            graph
                .analyze_energy(&input, &EnergyConfig::default())
                .is_ok()
        );

        check_logs(
            log::Level::Warn,
            vec![
                "group contains unconnected sub-trees. Analysis might not be complete.",
                "graph contains stale (completely unconnected) node 'stale node' (dummy). Skipping.",
            ],
        );
        Ok(())
    }

    fn prepare_group() -> OpmResult<OpticGraph> {
        let mut graph = OpticGraph::default();
        let g1_n1 = graph.add_node(Dummy::default())?;
        let g1_n2 = graph.add_node(BeamSplitter::new(
            "test",
            &SplittingConfigBuilder::FixedRatio(0.6),
        )?)?;

        graph.map_port(g1_n2, &PortType::Output, "out1_trans1_refl2", "output_1")?;
        graph.map_port(g1_n1, &PortType::Input, "input_1", "input_1")?;
        graph.connect_nodes(g1_n1, "output_1", g1_n2, "input_1", Length::zero())?;

        Ok(graph)
    }

    #[test]
    fn analyze_ok() -> OpmResult<()> {
        let mut graph = prepare_group()?;
        let mut input = LightResult::default();

        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_light.clone());

        let output = graph.analyze_energy(&input, &EnergyConfig::default())?;

        assert!(output.contains_key("output_1"));
        let output = output.get("output_1").unwrap().clone();
        let energy = if let LightData::Energy(s) = output {
            s.total_energy()
        } else {
            panic!()
        };
        assert_abs_diff_eq!(energy, 0.6);

        Ok(())
    }

    #[test]
    fn analyze_wrong_input_data() -> OpmResult<()> {
        let mut graph = prepare_group()?;
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("wrong".into(), input_light.clone());

        let output = graph.analyze_energy(&input, &EnergyConfig::default())?;
        assert!(output.is_empty());

        Ok(())
    }

    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut graph = prepare_group()?;
        let mut input = LightResult::default();

        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        graph.set_is_inverted(true);
        input.insert("output_1".into(), input_light);

        let output = graph.analyze_energy(&input, &EnergyConfig::default());
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("input_1"));
        let output = output.get("input_1").unwrap().clone();

        let energy = if let LightData::Energy(s) = output {
            s.total_energy()
        } else {
            panic!()
        };
        assert_abs_diff_eq!(energy, 0.6);

        Ok(())
    }

    #[test]
    fn analyze_inverse_with_src() -> OpmResult<()> {
        let mut graph = OpticGraph::default();
        let g1_n1 = graph.add_node(SourcePort::default())?;
        let g1_n2 = graph.add_node(Dummy::default())?;

        graph.map_port(g1_n2, &PortType::Output, "output_1", "output_1")?;
        graph.connect_nodes(g1_n1, "output_1", g1_n2, "input_1", Length::zero())?;

        graph.set_is_inverted(true);
        let mut input = LightResult::default();

        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light);

        let output = graph.analyze_energy(&input, &EnergyConfig::default());
        assert!(output.is_ok());

        Ok(())
    }
}
