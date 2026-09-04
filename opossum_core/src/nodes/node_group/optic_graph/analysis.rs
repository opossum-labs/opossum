use log::warn;
use nalgebra::Vector3;
use petgraph::{Direction, algo::toposort, graph::NodeIndex, visit::EdgeRef};
use uom::si::f64::Length;
use uuid::Uuid;

use crate::{
    analyzers::energy::{AnalysisEnergy, EnergyConfig},
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult},
    nodes::NodeGroup,
    utils::LockExt,
};

use super::OpticGraph;

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
            let node = self.node_by_idx(idx)?.optical_ref;
            let node_id = self.node_by_idx(idx)?.uuid()?;
            if self.is_stale_node(node_id)? {
                warn!(
                    "graph contains stale (completely unconnected) node {}. Skipping.",
                    node.lock_opm()?
                );
            } else {
                let incoming_edges = self.take_incoming(node_id, incoming_data)?;
                let node_name = format!("{}", node.lock_opm()?);
                let outgoing_edges =
                    AnalysisEnergy::analyze(&mut *node.lock_opm()?, incoming_edges, config)
                        .map_err(|e| {
                            OpossumError::Analysis(format!(
                                "analysis of node {node_name} failed: {e}"
                            ))
                        })?;
                // If node is sink node, rewrite port names according to output mapping
                if self.is_output_node(node_id)? {
                    let portmap = if self.is_inverted() {
                        &self.input_port_map
                    } else {
                        &self.output_port_map
                    };
                    let node_id = self.node_by_idx(idx)?.uuid()?;
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
    /// # Panics
    ///
    /// Panics if an error occurs while locking a mutex.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    /// - there is no connecting edge from a predecessor.
    /// - the length cannot be determined (e.g. predecessor node has no fixed isometry set).
    pub fn distance_from_predecessor(&self, node_id: Uuid, port_name: &str) -> OpmResult<Length> {
        let portmap = if self.is_inverted() {
            &self.output_port_map
        } else {
            &self.input_port_map
        };
        if let Some(external_port_name) = portmap.external_port_name(node_id, port_name) {
            self.external_distances().get(&external_port_name).map_or_else(|| Err(OpossumError::Analysis(format!("did not find distance from predecessor to target port '{port_name}' because it's not in the list of external distances"))), |length| Ok(*length))
        } else {
            let idx = self.node_idx_by_uuid(node_id).unwrap();
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
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the given `node_id` was not found in the graph.
    ///  - the given `incoming_edges` are not of type `LightData::Geometric`.
    ///  - the given `incoming_edges` contain no rays.
    ///  - the resulting isometry is inconsistent with a previously placed node.
    ///  - the mutex lock failed.
    pub fn set_node_isometry(
        &self,
        incoming_edges: &LightResult,
        node_id: Uuid,
        up_direction: Vector3<f64>,
    ) -> OpmResult<()> {
        for incoming_edge in incoming_edges {
            let node_ref = self.node(node_id)?;
            let distance_from_predecessor =
                self.distance_from_predecessor(node_id, incoming_edge.0)?;
            let mut node = node_ref.optical_ref.lock_opm()?;
            if let Some(group) = node.as_any_mut().downcast_mut::<NodeGroup>() {
                group.add_input_port_distance(incoming_edge.0, distance_from_predecessor);
            }
            let LightData::Geometric(rays) = incoming_edge.1 else {
                return Err(OpossumError::Analysis(
                    "expected LightData::Geometric at input port".into(),
                ));
            };
            if let Some(ray) = rays.into_iter().next() {
                let mut ray = ray.to_owned();
                ray.propagate(distance_from_predecessor)?;
                let node_iso = ray.to_isometry(up_direction);
                // if a node with more than one input was already placed (in an earlier loop cycle),
                // check, if the resulting isometry is consistent
                {
                    if let Some(iso) = node.isometry() {
                        if iso != node_iso {
                            warn!("Node {} cannot be consistently positioned.", node.name());
                            warn!("Position based on previous input port is: {iso}");
                            warn!("Position based on this port would be:     {node_iso}");
                            warn!("Keeping first position");
                        }
                    } else {
                        node.set_isometry(node_iso)?;
                        drop(node);
                    }
                }
            } else {
                return Err(OpossumError::Analysis(
                    "no rays in this ray bundle. cannot position nodes".into(),
                ));
            }
        }
        Ok(())
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
        dummy.set_isometry(Isometry::identity())?;
        let d1 = graph.add_node(dummy)?;
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity())?;
        let d2 = graph.add_node(dummy)?;
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity())?;
        let d3 = graph.add_node(dummy)?;
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity())?;
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
        dummy.set_isometry(Isometry::identity())?;
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
