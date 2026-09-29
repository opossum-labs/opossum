use log::warn;
use nalgebra::Vector3;
use petgraph::{Direction, algo::toposort, graph::NodeIndex, visit::EdgeRef};
use uom::si::{
    angle::degree,
    f64::Length,
    length::{meter, millimeter},
};
use uuid::Uuid;

use crate::{
    analyzers::energy::{AnalysisEnergy, EnergyConfig},
    core_optics::{NodeAttrExt, PortType, node_attr::NodePositioning},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult},
    nanometer,
    nodes::NodeGroup,
    radian,
    utils::geom_transformation::{AxisMismatch, Isometry},
};

use super::OpticGraph;

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
    let start_x = (origin.x.get::<meter>() - d * direction.x) * 1.0e3;
    let start_y = (origin.y.get::<meter>() - d * direction.y) * 1.0e3;
    let start_z = (origin.z.get::<meter>() - d * direction.z) * 1.0e3;
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
    /// Returns the incoming data of a node in this [`OpticGraph`].
    ///
    /// This function returns the incoming data of a node with the given [`Uuid`]. If the node is an external node, the
    /// incoming data is mapped to the internal node names.
    ///
    /// # Errors
    ///
    /// This functions returns an error if the given `node_id` does not exist.
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
            // map group-external data and add
            for incoming in incoming_data {
                if let Some(mapping) = portmap.get(incoming.0)
                    && node_id == mapping.0
                {
                    mapped_light_result.insert(mapping.1.clone(), incoming.1.clone());
                }
            }
            // add group internal data
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
    /// incoming data is mapped to the internal node names. This function is similar to `get_incoming` but it has move semantic.
    ///
    /// # Errors
    ///
    /// This functions returns an error if the given `node_id` does not exist.
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

            // For external data we still to clone (since it might be reused)
            // Maybe we can optimize that later
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
    // helper function: Move data out of an edge
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
    /// Returns the topologically sorted of this [`OpticGraph`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn topologically_sorted(&self) -> OpmResult<Vec<NodeIndex>> {
        toposort(&self.g, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))
    }
    /// Performs an energy flow analysis of this graph.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn analyze_energy(
        &mut self,
        incoming_data: &LightResult,
        config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        if self.is_inverted() {
            self.invert_graph()?;
        }
        if !self.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }
        let sorted = self.topologically_sorted()?;
        let mut light_result = LightResult::default();
        for idx in sorted {
            let node_id = self.node_by_idx(idx)?.uuid();
            if self.is_stale_node(node_id)? {
                let node_name = format!("{}", self.g[idx]);
                warn!("graph contains stale (completely unconnected) node {node_name}. Skipping.");
            } else {
                let incoming_edges = self.take_incoming(node_id, incoming_data)?;
                let outgoing_edges = if let Some(target_uuid) = self.g[idx].referenced_node_id() {
                    let is_inverted = self.g[idx].inverted();
                    let target_idx = self.node_idx_by_uuid(target_uuid).ok_or_else(|| {
                        OpossumError::Analysis(format!(
                            "referenced node with id {target_uuid} not found in graph"
                        ))
                    })?;
                    let target_node = &mut self.g[target_idx];
                    if is_inverted {
                        target_node.set_inverted(true).map_err(|_e| {
                            OpossumError::Analysis(format!(
                                "referenced node {target_node} cannot be inverted"
                            ))
                        })?;
                    }
                    let res = AnalysisEnergy::analyze(&mut **target_node, incoming_edges, config);
                    if is_inverted {
                        target_node.set_inverted(false)?;
                    }
                    let node_name = format!("{target_node}");
                    res.map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                    })?
                } else {
                    let node = &mut self.g[idx];
                    let node_name = format!("{node}");
                    AnalysisEnergy::analyze(&mut **node, incoming_edges, config).map_err(|e| {
                        OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                    })?
                };
                // If node is sink node, rewrite port names according to output mapping
                if self.is_output_node(node_id)? {
                    let portmap = if self.is_inverted() {
                        &self.input_port_map
                    } else {
                        &self.output_port_map
                    };
                    let assigned_ports = portmap.assigned_ports_for_node(node_id);
                    for port in assigned_ports {
                        if let Some(light_data) = outgoing_edges.get(&port.1) {
                            light_result.insert(port.0, light_data.clone());
                        }
                    }
                }
                for outgoing_edge in outgoing_edges {
                    self.set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);
                }
            }
        }
        if self.is_inverted() {
            self.invert_graph()?;
        } // revert initial inversion (if necessary)
        Ok(light_result)
    }
    /// Returns the (optical) distance to a connected predecessor node.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - the node with the given `node_id` does not exist in the graph.
    /// - there is no connecting edge from a predecessor.
    /// - the length cannot be determined (e.g. predecessor node has no fixed isometry set).
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
            // Safely resolve the node index or propagate an analysis error
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
    /// This function will return an error if
    ///  - the given `node_id` was not found in the graph.
    ///  - the given `incoming_edges` are not of type `LightData::Geometric`.
    ///  - the given `incoming_edges` contain no rays.
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
            match self.g[idx].positioning().effective_position().copied() {
                // Node already placed: check this input's axis against the placement. The axis should
                // reach the port's entrance frame `F ∘ L` placed in the world; compare it with the
                // ray actually arriving there.
                Some(placed_iso) => {
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
                }
                // Node not yet placed: its frame `F` satisfies `F ∘ L = W` (the world frame `W` of the
                // entering axis ray equals the port's local entrance frame `L` placed in the world),
                // hence `F = W ∘ L⁻¹`. `L` is the identity for ordinary ports (so `F = W`) and mirrored
                // for a beam splitter's second input.
                None => {
                    let node_iso = world_frame.append(&Isometry::new_from_transform(
                        entrance_frame.get_inv_transform(),
                    ));
                    self.g[idx].set_positioning(NodePositioning::Automatic(Some(node_iso)))?;
                    primary_port = Some(port_name.clone());
                }
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
    /// Returns true if data has been passed on, false otherwise
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
        utils::{geom_transformation::Isometry, test_helper::test_helper::check_logs},
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
