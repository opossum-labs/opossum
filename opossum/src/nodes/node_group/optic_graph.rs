#![warn(missing_docs)]
use crate::{
    analyzers::{energy::AnalysisEnergy, Analyzable},
    error::{OpmResult, OpossumError},
    light_flow::LightFlow,
    light_result::LightResult,
    lightdata::LightData,
    nodes::NodeGroup,
    optic_node::OpticNode,
    optic_ports::PortType,
    optic_ref::OpticRef,
    optic_senery_rsc::SceneryResources,
    port_map::PortMap,
    properties::{proptype::format_quantity, Proptype},
    rays::Rays,
};
use log::warn;
use nalgebra::Vector3;
use petgraph::{
    algo::{connected_components, is_cyclic_directed, toposort},
    graph::{EdgeIndex, Edges},
    prelude::DiGraph,
    stable_graph::NodeIndex,
    visit::EdgeRef,
    Directed, Direction,
};
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};
use std::{
    cell::{RefCell, RefMut},
    collections::BTreeMap,
    rc::Rc,
};
use uom::si::{f64::Length, length::meter};
use uuid::Uuid;

/// Data structure representing an optical graph
#[derive(Debug, Default, Clone)]
pub struct OpticGraph {
    g: DiGraph<OpticRef, LightFlow>,
    input_port_map: PortMap,
    output_port_map: PortMap,
    is_inverted: bool,
    external_distances: BTreeMap<String, Length>,
    global_confg: Option<Rc<RefCell<SceneryResources>>>,
}
impl OpticGraph {
    /// Add a new optical node to this [`OpticGraph`].
    ///
    /// This function returns a [`NodeIndex`] of the added node for later referencing (see `connect_nodes`).
    /// **Note**: While constructing the underlying [`OpticRef`] a random, uuid is assigned.
    ///
    /// # Errors
    /// This function returns an error if the graph is set as `inverted` and a node is added. (This could end up in
    /// a weird / undefined behaviour)
    pub fn add_node<T: Analyzable + 'static>(&mut self, node: T) -> OpmResult<NodeIndex> {
        if self.is_inverted {
            return Err(OpossumError::OpticGroup(
                "cannot add nodes if group is set as inverted".into(),
            ));
        }
        let idx = self.g.add_node(OpticRef::new(
            Rc::new(RefCell::new(node)),
            self.global_confg.clone(),
        ));

        Ok(idx)
    }
    /// Connect two optical nodes within this [`OpticGraph`].
    ///
    /// This function connects two optical nodes (referenced by their [`NodeIndex`]) with their respective port names and their geometrical distance
    /// (= propagation length) to each other thus extending the network.
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the [`NodeIndex`] of source or target node does not exist in the [`OpticGraph`]
    ///   - a port name of the source or target node does not exist
    ///   - if a node/port combination was already connected earlier
    ///   - the connection of the nodes would form a loop in the network.
    ///   - the given geometric distance between the nodes is not finite.
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        if self.is_inverted {
            return Err(OpossumError::OpticGroup(
                "cannot connect nodes if group is set as inverted".into(),
            ));
        }
        let source = self.g.node_weight(src_node).ok_or_else(|| {
            OpossumError::OpticScenery("source node with given index does not exist".into())
        })?;
        if !source
            .optical_ref
            .borrow()
            .ports()
            .names(&PortType::Output)
            .contains(&src_port.into())
        {
            return Err(OpossumError::OpticScenery(format!(
                "source node {} does not have a port {}",
                source.optical_ref.borrow(),
                src_port
            )));
        }
        let target = self.g.node_weight(target_node).ok_or_else(|| {
            OpossumError::OpticScenery("target node with given index does not exist".into())
        })?;
        if !target
            .optical_ref
            .borrow()
            .ports()
            .names(&PortType::Input)
            .contains(&target_port.into())
        {
            return Err(OpossumError::OpticScenery(format!(
                "target node {} does not have a port {}",
                target.optical_ref.borrow(),
                target_port
            )));
        }
        if self.src_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "src node <{}> with port <{}> is already connected",
                source.optical_ref.borrow(),
                src_port
            )));
        }
        if self.target_node_port_exists(target_node, target_port) {
            return Err(OpossumError::OpticScenery(format!(
                "target node {} with port <{}> is already connected",
                target.optical_ref.borrow(),
                target_port
            )));
        }
        let src_name = source.optical_ref.borrow().name();
        let target_name = target.optical_ref.borrow().name();
        let light = LightFlow::new(src_port, target_port, distance)?;
        let edge_index = self.g.add_edge(src_node, target_node, light);
        if is_cyclic_directed(&self.g) {
            self.g.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(format!(
                "connecting nodes <{src_name}> -> <{target_name}> would form a loop"
            )));
        }
        // remove input port mapping, if no loner valid
        self.input_port_map.remove_mapping(target_node, target_port);
        // remove output port mapping, if no loner valid
        self.output_port_map.remove_mapping(src_node, src_port);
        Ok(())
    }
    /// Returns a reference to the input port map of this [`OpticGraph`].
    #[must_use]
    pub const fn port_map(&self, port_type: &PortType) -> &PortMap {
        match port_type {
            PortType::Input => &self.input_port_map,
            PortType::Output => &self.output_port_map,
        }
    }
    fn external_nodes(&self, port_type: &PortType) -> Vec<NodeIndex> {
        let edge_direction = match port_type {
            PortType::Input => Direction::Incoming,
            PortType::Output => Direction::Outgoing,
        };
        let mut nodes: Vec<NodeIndex> = Vec::default();
        for node_idx in self.g.node_indices() {
            let edges = self.edges_directed(node_idx, edge_direction).count();
            let ports = self
                .node_by_idx(node_idx)
                .unwrap()
                .optical_ref
                .borrow()
                .ports()
                .names(port_type)
                .len();
            if ports != edges {
                nodes.push(node_idx);
            }
        }
        nodes
    }
    /// Map a port of an internal node to an external port of the group.
    ///
    /// In oder to use an [`OpticGraph`] from the outside, internal nodes / ports must be mapped to be visible. The
    /// corresponding `ports` function only returns ports that have been mapped before.
    /// # Errors
    ///
    /// This function will return an error if
    ///   - an external input port name has already been assigned.
    ///   - the `input_node` / `internal_name` does not exist.
    ///   - the specified `input_node` is not an input node of the group (i.e. fully connected to other internal nodes).
    ///   - the `input_node` has an input port with the specified `internal_name` but is already internally connected.
    pub fn map_port(
        &mut self,
        node_idx: NodeIndex,
        port_type: &PortType,
        internal_name: &str,
        external_name: &str,
    ) -> OpmResult<()> {
        let name_type = match port_type {
            PortType::Input => "input_1",
            PortType::Output => "output_1",
        };
        let port_map = match port_type {
            PortType::Input => &self.input_port_map,
            PortType::Output => &self.output_port_map,
        };
        if port_map.contains_external_name(external_name) {
            return Err(OpossumError::OpticGroup(format!(
                "external {name_type} port name already assigned"
            )));
        }
        let node = self.node_by_idx(node_idx)?;
        if !node
            .optical_ref
            .borrow()
            .ports()
            .names(port_type)
            .contains(&(internal_name.to_string()))
        {
            return Err(OpossumError::OpticGroup(format!(
                "internal {name_type} port name not found"
            )));
        }
        if !self.external_nodes(port_type).contains(&node_idx) {
            return Err(OpossumError::OpticGroup(format!(
                "node to be mapped is not an {name_type} node of the group"
            )));
        }
        let edge_direction = match port_type {
            PortType::Input => Direction::Incoming,
            PortType::Output => Direction::Outgoing,
        };

        let edge_connected = match port_type {
            PortType::Input => self
                .g
                .edges_directed(node_idx, edge_direction)
                .map(|e| e.weight().target_port())
                .any(|p| p == internal_name),
            PortType::Output => self
                .g
                .edges_directed(node_idx, edge_direction)
                .map(|e| e.weight().src_port())
                .any(|p| p == internal_name),
        };
        if edge_connected {
            return Err(OpossumError::OpticGroup(format!(
                "port of {name_type} node is already internally connected"
            )));
        }
        match port_type {
            PortType::Input => self
                .input_port_map
                .add(external_name, node_idx, internal_name),
            PortType::Output => self
                .output_port_map
                .add(external_name, node_idx, internal_name),
        };
        Ok(())
    }
    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    #[must_use]
    pub fn get_incoming(&self, idx: NodeIndex, incoming_data: &LightResult) -> LightResult {
        if self.is_incoming_node(idx) {
            let portmap = if self.is_inverted {
                self.output_port_map.clone()
            } else {
                self.input_port_map.clone()
            };
            let mut mapped_light_result = LightResult::default();
            // map group-external data and add
            for incoming in incoming_data {
                if let Some(mapping) = portmap.get(incoming.0) {
                    if idx == mapping.0 {
                        mapped_light_result.insert(mapping.1.clone(), incoming.1.clone());
                    }
                }
            }
            // add group internal data
            for edge in self.incoming_edges(idx) {
                mapped_light_result.insert(edge.0.clone(), edge.1.clone());
            }
            mapped_light_result
        } else {
            self.incoming_edges(idx)
        }
    }

    ///Clear the edges of an optic graph. Useful for back- and forth-propagation in ghost focus analysis
    pub fn clear_edges(&mut self) {
        let node_indices = self.g.node_indices();
        for idx in node_indices {
            let mut ids = Vec::<EdgeIndex>::new();
            for edge in self.edges_directed(idx, Direction::Incoming) {
                ids.push(edge.id());
            }
            for id in ids {
                let light = self.g.edge_weight_mut(id);
                if let Some(light) = light {
                    light.set_data(Some(LightData::Geometric(Rays::default())));
                }
            }

            let mut ids = Vec::<EdgeIndex>::new();
            for edge in self.edges_directed(idx, Direction::Outgoing) {
                ids.push(edge.id());
            }
            for id in ids {
                let light = self.g.edge_weight_mut(id);
                if let Some(light) = light {
                    light.set_data(Some(LightData::Geometric(Rays::default())));
                }
            }
        }
    }
    /// .
    #[must_use]
    pub fn is_stale_node(&self, idx: NodeIndex) -> bool {
        let neighbors = self.g.neighbors_undirected(idx);
        neighbors.count() == 0 && !self.input_port_map.contains_node(idx)
    }
    /// Update reference to global config for each node in this [`OpticGraph`].
    /// This function is needed after deserialization.
    pub fn update_global_config(&mut self, global_conf: &Option<Rc<RefCell<SceneryResources>>>) {
        for node in self.g.node_weights_mut() {
            node.update_global_config(global_conf.clone());
        }
    }
    fn src_node_port_exists(&self, src_node: NodeIndex, src_port: &str) -> bool {
        self.g
            .edges_directed(src_node, petgraph::Direction::Outgoing)
            .any(|e| e.weight().src_port() == src_port)
    }
    fn target_node_port_exists(&self, target_node: NodeIndex, target_port: &str) -> bool {
        self.g
            .edges_directed(target_node, petgraph::Direction::Incoming)
            .any(|e| e.weight().target_port() == target_port)
    }
    /// Returns Some(`OpticRef`) if the provided uuid is connected with a node in the graph. None, otherwise.
    #[must_use]
    pub fn node_by_uuid(&self, uuid: Uuid) -> Option<OpticRef> {
        self.g
            .node_weights()
            .find(|node| node.uuid() == uuid)
            .cloned()
    }
    /// Return a reference to the optical node specified by its node index.
    ///
    /// This function is mainly useful for setting up a reference node.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_by_idx(&self, node: NodeIndex) -> OpmResult<OpticRef> {
        let node = self
            .g
            .node_weight(node)
            .ok_or_else(|| OpossumError::OpticScenery("node index does not exist".into()))?;
        Ok(node.clone())
    }

    /// Return a mutable reference to the optical node specified by its node index.
    ///
    /// This function is mainly useful for setting up a reference node.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node_by_idx_mut(&mut self, node: NodeIndex) -> OpmResult<&mut OpticRef> {
        let node = self
            .g
            .node_weight_mut(node)
            .ok_or_else(|| OpossumError::OpticScenery("node index does not exist".into()))?;
        Ok(node)
    }
    fn node_idx_by_uuid(&self, uuid: Uuid) -> Option<NodeIndex> {
        self.g
            .node_indices()
            .find(|idx| self.g.node_weight(*idx).unwrap().uuid() == uuid)
    }
    /// Returns all nodes ([`OpticRef`]) of this [`OpticGraph`].
    #[must_use]
    pub fn nodes(&self) -> Vec<&OpticRef> {
        self.g.node_weights().collect()
    }
    fn edge_by_idx(&self, idx: EdgeIndex) -> OpmResult<&LightFlow> {
        self.g
            .edge_weight(idx)
            .ok_or_else(|| OpossumError::Other("could not get edge weight".into()))
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
    pub fn analyze_energy(&mut self, incoming_data: &LightResult) -> OpmResult<LightResult> {
        if self.is_inverted() {
            self.invert_graph()?;
        }
        let g_clone = self.clone();
        if !self.is_single_tree() {
            warn!("group contains unconnected sub-trees. Analysis might not be complete.");
        }
        let sorted = self.topologically_sorted()?;
        let mut light_result = LightResult::default();
        for idx in sorted {
            let node = g_clone.node_by_idx(idx)?.optical_ref;
            if self.is_stale_node(idx) {
                warn!(
                    "graph contains stale (completely unconnected) node {}. Skipping.",
                    node.borrow()
                );
            } else {
                let incoming_edges = self.get_incoming(idx, incoming_data);
                let node_name = format!("{}", node.borrow());
                let outgoing_edges = AnalysisEnergy::analyze(
                    &mut *node.borrow_mut(),
                    incoming_edges,
                )
                .map_err(|e| {
                    OpossumError::Analysis(format!("analysis of node {node_name} failed: {e}"))
                })?;
                // If node is sink node, rewrite port names according to output mapping
                if self.is_output_node(idx) {
                    let portmap = if self.is_inverted {
                        self.input_port_map.clone()
                    } else {
                        self.output_port_map.clone()
                    };
                    let assigned_ports = portmap.assigned_ports_for_node(idx);
                    for port in assigned_ports {
                        if let Some(light_data) = outgoing_edges.get(&port.1) {
                            light_result.insert(port.0, light_data.clone());
                        }
                    }
                }
                for outgoing_edge in outgoing_edges {
                    self.set_outgoing_edge_data(idx, &outgoing_edge.0, &outgoing_edge.1);
                }
            }
        }
        if self.is_inverted {
            self.invert_graph()?;
        } // revert initial inversion (if necessary)
        Ok(light_result)
    }
    /// Returns the is single tree of this [`OpticGraph`].
    #[must_use]
    pub fn is_single_tree(&self) -> bool {
        connected_components(&self.g) == 1
    }
    /// Returns the number of nodes in this [`OpticGraph`].
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.g.node_count()
    }
    /// Returns the number of connection (edges) in this [`OpticGraph`].
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.g.edge_count()
    }
    fn is_incoming_node(&self, idx: NodeIndex) -> bool {
        let nr_of_input_ports = self
            .node_by_idx(idx)
            .unwrap()
            .optical_ref
            .borrow()
            .ports()
            .ports(&PortType::Input)
            .len();
        let nr_of_incoming_edges = self.g.edges_directed(idx, Direction::Incoming).count();
        assert!(
            nr_of_incoming_edges <= nr_of_input_ports,
            "# of incoming edges > # of input ports ???"
        );
        nr_of_incoming_edges < nr_of_input_ports
    }
    /// .
    ///
    /// # Panics
    ///
    /// Panics if .
    #[must_use]
    pub fn is_output_node(&self, idx: NodeIndex) -> bool {
        let ports = self.node_by_idx(idx).unwrap().optical_ref.borrow().ports();
        let nr_of_output_ports = ports.ports(&PortType::Output).len();
        let nr_of_outgoing_edges = self.g.edges_directed(idx, Direction::Outgoing).count();
        debug_assert!(
            nr_of_outgoing_edges <= nr_of_output_ports,
            "# of outgoing edges > # of output ports ???"
        );
        nr_of_outgoing_edges < nr_of_output_ports
    }
    fn incoming_edges(&self, idx: NodeIndex) -> LightResult {
        let edges = self.g.edges_directed(idx, Direction::Incoming);
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
    /// Sets the outgoing edge data of this [`OpticGraph`].
    /// Returns true if data has been passed on, false otherwise
    pub fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: &str, data: &LightData) -> bool {
        let edges = self.g.edges_directed(idx, Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_idx = edge_ref.id();
            let light = self.g.edge_weight_mut(edge_idx);
            if let Some(light) = light {
                light.set_data(Some(data.clone()));
            }
            true
        }
        // else outgoing edge not connected -> data dropped
        else {
            false
        }
    }
    fn edges_directed(&self, idx: NodeIndex, dir: Direction) -> Edges<'_, LightFlow, Directed> {
        self.g.edges_directed(idx, dir)
    }
    /// Inverts the [`OpticGraph`].
    ///
    /// This functions changes all directions of node connections and inverts the nodes itself.
    /// # Errors
    ///
    /// This function will return an error if one tries to invert a graph containing a non-invertable node (eg. source).
    pub fn invert_graph(&mut self) -> OpmResult<()> {
        for node in self.g.node_weights_mut() {
            let node_to_be_inverted = !node.optical_ref.borrow().inverted();

            node.optical_ref
                .borrow_mut()
                .set_inverted(node_to_be_inverted)
                .map_err(|_| {
                    OpossumError::OpticGroup(
                        "group cannot be inverted because it contains a non-invertable node".into(),
                    )
                })?;
        }
        for edge in self.g.edge_weights_mut() {
            edge.inverse();
        }
        self.g.reverse();
        Ok(())
    }
    /// Sets the node isometry of this [`OpticGraph`].
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn set_node_isometry(
        &self,
        incoming_edges: &LightResult,
        node_borrow_mut: &mut RefMut<'_, dyn Analyzable + 'static>,
        node_type: &str,
        idx: NodeIndex,
        up_direction: Vector3<f64>,
    ) -> OpmResult<()> {
        for incoming_edge in incoming_edges {
            let distance_from_predecessor = self.distance_from_predecessor(idx, incoming_edge.0)?;
            if node_type == "group" {
                // let mut node_borrow_mut = node;
                let group = node_borrow_mut.as_group()?;
                group.add_input_port_distance(incoming_edge.0, distance_from_predecessor);
            }
            if let LightData::Geometric(rays) = incoming_edge.1 {
                if rays.nr_of_rays(false) == 0 {
                    return Err(OpossumError::Analysis(
                        "no rays in this ray bundle. cannot position nodes".into(),
                    ));
                }
                let mut ray = rays.into_iter().next().unwrap().to_owned();
                ray.propagate(distance_from_predecessor)?;
                let node_iso = ray.to_isometry(up_direction);
                // if a node with more than one input was already placed (in an earlier loop cycle),
                // check, if the resulting isometry is consistent
                {
                    if let Some(iso) = node_borrow_mut.isometry() {
                        if iso != node_iso {
                            warn!(
                                "Node {} cannot be consistently positioned.",
                                node_borrow_mut.name()
                            );
                            warn!("Position based on previous input port is: {iso}");
                            warn!("Position based on this port would be:     {node_iso}");
                            warn!("Keeping first position");
                        }
                    } else {
                        node_borrow_mut.set_isometry(node_iso)?;
                    };
                }
            } else {
                return Err(OpossumError::Analysis(
                    "expected LightData::Geometric at input port".into(),
                ));
            }
        }
        Ok(())
    }
    /// Creates the dot-format string which describes the edge that connects two nodes
    ///
    /// # Parameters:
    /// * `end_node_idx`:         [`NodeIndex`] of the node that should be connected
    /// * `light_port`:           port name that should be connected
    ///
    /// Returns the result of the edge strnig for the dot format
    fn create_node_edge_str(&self, end_node_idx: NodeIndex, light_port: &str) -> OpmResult<String> {
        let node_id = format!("i{}", self.node_by_idx(end_node_idx)?.uuid().as_simple());
        let node = self.node_by_idx(end_node_idx)?;
        if node.optical_ref.borrow().node_type() == "group" {
            let mut node = node.optical_ref.borrow_mut();
            let group_node: &NodeGroup = node.as_group()?;
            Ok(group_node.get_mapped_port_str(light_port, &node_id)?)
        } else {
            Ok(format!("{node_id}:{light_port}"))
        }
    }
    /// Retruns a string of a graphwiz structure of this group.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn create_dot_string(&self, rankdir: &str) -> OpmResult<String> {
        //check direction
        let rankdir = if rankdir == "LR" { "LR" } else { "TB" };
        let mut dot_string = String::default();
        let sorted = self.topologically_sorted()?;
        for idx in &sorted {
            let node = self.node_by_idx(*idx)?;
            let node_name = node.optical_ref.borrow().name();
            let inverted = node.optical_ref.borrow().node_attr().inverted();
            let ports = node.optical_ref.borrow().ports();
            dot_string += &node.optical_ref.borrow().to_dot(
                &format!("{}", node.uuid().as_simple()),
                &node_name,
                inverted,
                &ports,
                rankdir,
            )?;
        }
        for edge_idx in self.g.edge_indices() {
            let light: &LightFlow = self.edge_by_idx(edge_idx)?;
            let end_nodes = self
                .g
                .edge_endpoints(edge_idx)
                .ok_or_else(|| OpossumError::Other("could not get edge_endpoints".into()))?;

            let dist = self.distance_from_predecessor(end_nodes.1, light.target_port())?;

            let src_edge_str = self.create_node_edge_str(end_nodes.0, light.src_port())?;
            let target_edge_str = self.create_node_edge_str(end_nodes.1, light.target_port())?;

            dot_string.push_str(&format!(
                "  {src_edge_str} -> {target_edge_str} [label=\"{}\"]\n",
                format_quantity(meter, dist)
            ));
        }
        dot_string.push_str("}\n");
        Ok(dot_string)
    }
    fn distance_from_predecessor(&self, idx: NodeIndex, port_name: &str) -> OpmResult<Length> {
        let portmap = if self.is_inverted {
            self.output_port_map.clone()
        } else {
            self.input_port_map.clone()
        };
        if let Some(external_port_name) = portmap.external_port_name(idx, port_name) {
            self.external_distances.get(&external_port_name).map_or_else(|| Err(OpossumError::Analysis(format!("did not find distance from predecessor to target port '{port_name}' because it's not in the list of external distances"))), |length| Ok(*length))
        } else {
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
    /// Returns `true` if the graph is inverted.
    #[must_use]
    pub const fn is_inverted(&self) -> bool {
        self.is_inverted
    }
    /// Sets the is inverted of this [`OpticGraph`].
    pub fn set_is_inverted(&mut self, is_inverted: bool) {
        self.is_inverted = is_inverted;
    }
    /// Sets the external distances of this [`OpticGraph`].
    pub fn set_external_distances(&mut self, external_distances: BTreeMap<String, Length>) {
        self.external_distances = external_distances;
    }
}
impl Serialize for OpticGraph {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let g = self.g.clone();
        let mut graph = serializer.serialize_struct("graph", 4)?;
        let nodes = g.node_weights().cloned().collect::<Vec<OpticRef>>();
        graph.serialize_field("nodes", &nodes)?;
        let edgeidx = g
            .edge_indices()
            .map(|e| {
                (
                    g.node_weight(g.edge_endpoints(e).unwrap().0)
                        .unwrap()
                        .uuid(),
                    g.node_weight(g.edge_endpoints(e).unwrap().1)
                        .unwrap()
                        .uuid(),
                    g.edge_weight(e).unwrap().src_port(),
                    g.edge_weight(e).unwrap().target_port(),
                    *g.edge_weight(e).unwrap().distance(),
                )
            })
            .collect::<Vec<EdgeInfo<'_>>>();
        graph.serialize_field("edges", &edgeidx)?;
        graph.serialize_field("input_map", &self.input_port_map)?;
        graph.serialize_field("output_map", &self.output_port_map)?;
        graph.end()
    }
}

type EdgeInfo<'a> = (Uuid, Uuid, &'a str, &'a str, Length);

impl<'de> Deserialize<'de> for OpticGraph {
    #[allow(clippy::too_many_lines)]

    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            Nodes,
            Edges,
            InputPortMap,
            OutputPortMap,
        }
        const FIELDS: &[&str] = &["nodes", "edges", "input_map", "output_map"];

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("`nodes`, `edges`, `input_map`, or `output_map`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "nodes" => Ok(Field::Nodes),
                            "edges" => Ok(Field::Edges),
                            "input_map" => Ok(Field::InputPortMap),
                            "output_map" => Ok(Field::OutputPortMap),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct OpticGraphVisitor;

        impl<'de> Visitor<'de> for OpticGraphVisitor {
            type Value = OpticGraph;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an OpticGraph")
            }
            fn visit_map<A>(self, mut map: A) -> std::result::Result<OpticGraph, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut g = OpticGraph::default();
                let mut nodes: Option<Vec<OpticRef>> = None;
                let mut edges: Option<Vec<EdgeInfo<'_>>> = None;
                let mut input_map: Option<PortMap> = None;
                let mut output_map: Option<PortMap> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Nodes => {
                            if nodes.is_some() {
                                return Err(de::Error::duplicate_field("nodes"));
                            }
                            nodes = Some(map.next_value::<Vec<OpticRef>>()?);
                        }
                        Field::Edges => {
                            if edges.is_some() {
                                return Err(de::Error::duplicate_field("edges"));
                            }
                            edges = Some(map.next_value::<Vec<EdgeInfo<'_>>>()?);
                        }
                        Field::InputPortMap => {
                            if input_map.is_some() {
                                return Err(de::Error::duplicate_field("input_map"));
                            }
                            input_map = Some(map.next_value::<PortMap>()?);
                        }
                        Field::OutputPortMap => {
                            if output_map.is_some() {
                                return Err(de::Error::duplicate_field("output_map"));
                            }
                            output_map = Some(map.next_value::<PortMap>()?);
                        }
                    }
                }
                let nodes = nodes.ok_or_else(|| de::Error::missing_field("nodes"))?;
                let edges = edges.ok_or_else(|| de::Error::missing_field("edges"))?;
                for node in &nodes {
                    g.g.add_node(node.clone());
                }
                // assign references to ref nodes (if any)
                for node in &nodes {
                    if node.optical_ref.borrow().node_type() == "reference" {
                        let mut my_node = node.optical_ref.borrow_mut();
                        let refnode = my_node.as_refnode_mut().unwrap();
                        let node_props = refnode.properties().clone();
                        let uuid =
                            if let Proptype::Uuid(uuid) = node_props.get("reference id").unwrap() {
                                *uuid
                            } else {
                                Uuid::nil()
                            };
                        let Some(reference_node) = g.node_by_uuid(uuid) else {
                            return Err(de::Error::custom(
                                "reference node found, which does not reference anything",
                            ));
                        };
                        let ref_name =
                            format!("ref ({})", reference_node.optical_ref.borrow().name());
                        refnode.assign_reference(&reference_node);

                        refnode.node_attr_mut().set_name(&ref_name);
                    }
                }
                for edge in &edges {
                    let src_idx = g.node_idx_by_uuid(edge.0).ok_or_else(|| {
                        de::Error::custom(format!("src id {} does not exist", edge.0))
                    })?;
                    let target_idx = g.node_idx_by_uuid(edge.1).ok_or_else(|| {
                        de::Error::custom(format!("target id {} does not exist", edge.1))
                    })?;
                    g.connect_nodes(src_idx, edge.2, target_idx, edge.3, edge.4)
                        .map_err(|e| {
                            de::Error::custom(format!("connecting OpticGraph nodes failed: {e}"))
                        })?;
                }
                if let Some(input_map) = input_map {
                    // todo: do sanity check
                    g.input_port_map = input_map;
                }
                if let Some(output_map) = output_map {
                    // todo: do sanity check
                    g.output_port_map = output_map;
                }
                Ok(g)
            }
        }
        deserializer.deserialize_struct("OpticGraph", FIELDS, OpticGraphVisitor)
    }
}
impl From<OpticGraph> for Proptype {
    fn from(value: OpticGraph) -> Self {
        Self::OpticGraph(value)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        lightdata::DataEnergy,
        millimeter,
        nodes::{BeamSplitter, Dummy, Source},
        ray::SplittingConfig,
        spectrum_helper::create_he_ne_spec,
        utils::{geom_transformation::Isometry, test_helper::test_helper::check_warnings},
    };
    use approx::assert_abs_diff_eq;
    use num::Zero;
    #[test]
    fn default() {
        let graph = OpticGraph::default();
        assert_eq!(graph.is_inverted, false);
        assert_eq!(graph.g.node_count(), 0)
    }
    #[test]
    fn add_node() {
        let mut og = OpticGraph::default();
        og.add_node(Dummy::default()).unwrap();
        assert_eq!(og.g.node_count(), 1);
    }
    #[test]
    fn add_node_inverted() {
        let mut og = OpticGraph::default();
        og.set_is_inverted(true);
        assert!(og.add_node(Dummy::default()).is_err());
    }
    #[test]
    fn connect_nodes_ok() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        assert!(graph
            .connect_nodes(n1, "output_1", n2, "input_1", Length::zero())
            .is_ok());
        assert_eq!(graph.g.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_wrong_ports() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        // wrong port names
        assert!(og
            .connect_nodes(sn1_i, "wrong", sn2_i, "input_1", Length::zero())
            .is_err());
        assert_eq!(og.g.edge_count(), 0);
        assert!(og
            .connect_nodes(sn1_i, "output_1", sn2_i, "wrong", Length::zero())
            .is_err());
        assert_eq!(og.g.edge_count(), 0);
    }
    #[test]
    fn connect_nodes_wrong_index() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        assert!(graph
            .connect_nodes(n1, "output_1", 5.into(), "input_1", Length::zero())
            .is_err());
        assert!(graph
            .connect_nodes(5.into(), "output_1", n2, "input_1", Length::zero())
            .is_err());
    }
    #[test]
    fn connect_nodes_wrong_distance() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        assert!(graph
            .connect_nodes(n1, "output_1", n2, "input_1", millimeter!(f64::NAN))
            .is_err());
        assert!(graph
            .connect_nodes(n1, "output_1", n2, "input_1", millimeter!(f64::INFINITY))
            .is_err());
        assert!(graph
            .connect_nodes(
                n1,
                "output_1",
                n2,
                "input_1",
                millimeter!(f64::NEG_INFINITY)
            )
            .is_err());
    }
    #[test]
    fn connect_nodes_target_already_connected() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        let n3 = graph.add_node(Dummy::default()).unwrap();
        assert!(graph
            .connect_nodes(n1, "output_1", n2, "input_1", Length::zero())
            .is_ok());
        assert!(graph
            .connect_nodes(n3, "output_1", n2, "input_1", Length::zero())
            .is_err());
        assert!(graph
            .connect_nodes(n1, "output_1", n3, "input_1", Length::zero())
            .is_err());
    }
    #[test]
    fn connect_nodes_loop_error() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        assert!(graph
            .connect_nodes(n1, "output_1", n2, "input_1", Length::zero())
            .is_ok());
        assert!(graph
            .connect_nodes(n2, "output_1", n1, "input_1", Length::zero())
            .is_err());
        assert_eq!(graph.g.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_inverted() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.set_is_inverted(true);
        assert!(og
            .connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .is_err());
    }
    #[test]
    fn connect_nodes_update_port_mapping() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();

        og.map_port(sn2_i, &PortType::Input, "input_1", "input_1")
            .unwrap();
        og.map_port(sn1_i, &PortType::Output, "output_1", "output_1")
            .unwrap();
        assert_eq!(og.input_port_map.len(), 1);
        assert_eq!(og.output_port_map.len(), 1);
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        // delete no longer valid port mapping
        assert_eq!(og.input_port_map.len(), 0);
        assert_eq!(og.output_port_map.len(), 0);
    }
    #[test]
    fn map_input_port() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        // wrong port name
        assert!(og
            .map_port(sn1_i, &PortType::Input, "wrong", "input_1")
            .is_err());
        assert_eq!(og.input_port_map.len(), 0);
        // wrong node index
        assert!(og
            .map_port(5.into(), &PortType::Input, "input_1", "input_1")
            .is_err());
        assert_eq!(og.input_port_map.len(), 0);
        // map output port
        assert!(og
            .map_port(sn2_i, &PortType::Input, "output_1", "input_1")
            .is_err());
        assert_eq!(og.input_port_map.len(), 0);
        // map internal node
        assert!(og
            .map_port(sn2_i, &PortType::Input, "input_1", "input_1")
            .is_err());
        assert_eq!(og.input_port_map.len(), 0);
        // correct usage
        assert!(og
            .map_port(sn1_i, &PortType::Input, "input_1", "input_1")
            .is_ok());
        assert_eq!(og.input_port_map.len(), 1);
    }
    #[test]
    fn map_input_port_half_connected_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(BeamSplitter::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input1", Length::zero())
            .unwrap();

        // node port already internally connected
        assert!(og
            .map_port(sn2_i, &PortType::Input, "input1", "bs_input")
            .is_err());

        // correct usage
        assert!(og
            .map_port(sn1_i, &PortType::Input, "input_1", "input_1")
            .is_ok());
        assert!(og
            .map_port(sn2_i, &PortType::Input, "input2", "bs_input")
            .is_ok());
        assert_eq!(og.input_port_map.len(), 2);
    }
    #[test]
    fn map_output_port() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();

        // wrong port name
        assert!(og
            .map_port(sn2_i, &PortType::Output, "wrong", "output_1")
            .is_err());
        assert_eq!(og.output_port_map.len(), 0);
        // wrong node index
        assert!(og
            .map_port(5.into(), &PortType::Output, "output_1", "output_1")
            .is_err());
        assert_eq!(og.output_port_map.len(), 0);
        // map input port
        assert!(og
            .map_port(sn1_i, &PortType::Output, "input_1", "output_1")
            .is_err());
        assert_eq!(og.output_port_map.len(), 0);
        // map internal node
        assert!(og
            .map_port(sn1_i, &PortType::Output, "output_1", "output_1")
            .is_err());
        assert_eq!(og.output_port_map.len(), 0);
        // correct usage
        assert!(og
            .map_port(sn2_i, &PortType::Output, "output_1", "output_1")
            .is_ok());
        assert_eq!(og.output_port_map.len(), 1);
    }
    #[test]
    fn map_output_port_half_connected_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(BeamSplitter::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "out1_trans1_refl2", sn2_i, "input_1", Length::zero())
            .unwrap();

        // node port already internally connected
        assert!(og
            .map_port(sn1_i, &PortType::Output, "out1_trans1_refl2", "bs_output")
            .is_err());

        // correct usage
        assert!(og
            .map_port(sn1_i, &PortType::Output, "out2_trans2_refl1", "bs_output")
            .is_ok());
        assert!(og
            .map_port(sn2_i, &PortType::Output, "output_1", "output_1")
            .is_ok());
        assert_eq!(og.output_port_map.len(), 2);
    }
    #[test]
    fn input_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sn2_i = og.add_node(Dummy::default()).unwrap();
        let sub_node3 = BeamSplitter::new("test", &SplittingConfig::Ratio(0.5)).unwrap();
        let sn3_i = og.add_node(sub_node3).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input_1", Length::zero())
            .unwrap();
        og.connect_nodes(sn2_i, "output_1", sn3_i, "input1", Length::zero())
            .unwrap();
        assert_eq!(
            og.external_nodes(&PortType::Input),
            vec![0.into(), 2.into()]
        )
    }
    #[test]
    fn output_nodes() {
        let mut og = OpticGraph::default();
        let sn1_i = og.add_node(Dummy::default()).unwrap();
        let sub_node1 = BeamSplitter::new("test", &SplittingConfig::Ratio(0.5)).unwrap();
        let sn2_i = og.add_node(sub_node1).unwrap();
        let sn3_i = og.add_node(Dummy::default()).unwrap();
        og.connect_nodes(sn1_i, "output_1", sn2_i, "input1", Length::zero())
            .unwrap();
        og.connect_nodes(sn2_i, "out1_trans1_refl2", sn3_i, "input_1", Length::zero())
            .unwrap();
        assert_eq!(
            og.external_nodes(&PortType::Input),
            vec![0.into(), 1.into()]
        )
    }
    #[test]
    fn node_by_uuid() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let uuid = graph.g.node_weight(n1).unwrap().uuid();
        assert!(graph.node_by_uuid(uuid).is_some());
        assert!(graph.node_by_uuid(Uuid::new_v4()).is_none());
    }
    #[test]
    fn node_id() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let uuid = graph.g.node_weight(n1).unwrap().uuid();
        assert_eq!(graph.node_idx_by_uuid(uuid), Some(n1));
        assert_eq!(graph.node_idx_by_uuid(Uuid::new_v4()), None);
    }
    #[test]
    fn is_single_tree() {
        let mut graph = OpticGraph::default();
        let n1 = graph.add_node(Dummy::default()).unwrap();
        let n2 = graph.add_node(Dummy::default()).unwrap();
        let n3 = graph.add_node(Dummy::default()).unwrap();
        let n4 = graph.add_node(Dummy::default()).unwrap();
        graph
            .connect_nodes(n1, "output_1", n2, "input_1", Length::zero())
            .unwrap();
        graph
            .connect_nodes(n3, "output_1", n4, "input_1", Length::zero())
            .unwrap();
        assert_eq!(graph.is_single_tree(), false);
        graph
            .connect_nodes(n2, "output_1", n3, "input_1", Length::zero())
            .unwrap();
        assert_eq!(graph.is_single_tree(), true);
    }
    #[test]
    fn analyze_empty() {
        let mut node = OpticGraph::default();
        let output = node.analyze_energy(&LightResult::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_subtree_warning() {
        let mut graph = OpticGraph::default();
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity()).unwrap();
        let d1 = graph.add_node(dummy.clone()).unwrap();
        let d2 = graph.add_node(dummy.clone()).unwrap();
        let d3 = graph.add_node(dummy.clone()).unwrap();
        let d4 = graph.add_node(dummy).unwrap();
        graph
            .connect_nodes(d1, "output_1", d2, "input_1", Length::zero())
            .unwrap();
        graph
            .connect_nodes(d3, "output_1", d4, "input_1", Length::zero())
            .unwrap();
        graph
            .map_port(d1, &PortType::Input, "input_1", "input_1")
            .unwrap();
        let input = LightResult::default();
        testing_logger::setup();
        graph.analyze_energy(&input).unwrap();
        check_warnings(vec![
            "group contains unconnected sub-trees. Analysis might not be complete.",
        ]);
    }
    #[test]
    fn analyze_stale_node() {
        let mut graph = OpticGraph::default();
        let mut dummy = Dummy::default();
        dummy.set_isometry(Isometry::identity()).unwrap();
        let d1 = graph.add_node(dummy).unwrap();
        let _ = graph.add_node(Dummy::new("stale node")).unwrap();
        graph
            .map_port(d1, &PortType::Input, "input_1", "input_1")
            .unwrap();
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Fourier);
        testing_logger::setup();
        assert!(graph.analyze_energy(&input).is_ok());
        check_warnings(vec![
            "group contains unconnected sub-trees. Analysis might not be complete.",
            "graph contains stale (completely unconnected) node 'stale node' (dummy). Skipping.",
        ]);
    }
    fn prepare_group() -> OpticGraph {
        let mut graph = OpticGraph::default();
        let g1_n1 = graph.add_node(Dummy::default()).unwrap();
        let g1_n2 = graph
            .add_node(BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap())
            .unwrap();
        graph
            .map_port(g1_n2, &PortType::Output, "out1_trans1_refl2", "output_1")
            .unwrap();
        graph
            .map_port(g1_n1, &PortType::Input, "input_1", "input_1")
            .unwrap();
        graph
            .connect_nodes(g1_n1, "output_1", g1_n2, "input1", Length::zero())
            .unwrap();
        graph
    }
    #[test]
    fn analyze_ok() {
        let mut graph = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input_1".into(), input_light.clone());
        let output = graph.analyze_energy(&input);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("output_1"));
        let output = output.get("output_1").unwrap().clone();
        let energy = if let LightData::Energy(data) = output {
            data.spectrum.total_energy()
        } else {
            panic!()
        };
        assert_abs_diff_eq!(energy, 0.6);
    }
    #[test]
    fn analyze_wrong_input_data() {
        let mut graph = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("wrong".into(), input_light.clone());
        let output = graph.analyze_energy(&input).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn analyze_inverse() {
        let mut graph = prepare_group();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        graph.set_is_inverted(true);
        input.insert("output_1".into(), input_light);
        let output = graph.analyze_energy(&input);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("input_1"));
        let output = output.get("input_1").unwrap().clone();
        let energy = if let LightData::Energy(data) = output {
            data.spectrum.total_energy()
        } else {
            panic!()
        };
        assert_abs_diff_eq!(energy, 0.6);
    }
    #[test]
    fn analyze_inverse_with_src() {
        let mut graph = OpticGraph::default();
        let g1_n1 = graph.add_node(Source::default()).unwrap();
        let g1_n2 = graph.add_node(Dummy::default()).unwrap();
        graph
            .map_port(g1_n2, &PortType::Output, "output_1", "output_1")
            .unwrap();
        graph
            .connect_nodes(g1_n1, "output_1", g1_n2, "input_1", Length::zero())
            .unwrap();
        graph.set_is_inverted(true);
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("output_1".into(), input_light);
        let output = graph.analyze_energy(&input);
        assert!(output.is_err());
    }
    #[test]
    fn serialize_deserialize() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        graph
            .map_port(i_d1, &PortType::Input, "input_1", "input1")
            .unwrap();
        graph
            .map_port(i_d2, &PortType::Input, "input_1", "input2")
            .unwrap();
        assert_eq!(
            graph.port_map(&PortType::Input).port_names(),
            vec!["input1", "input2"]
        );
        let serialized = serde_yaml::to_string(&graph).unwrap();
        let deserialized: OpticGraph = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized.port_map(&PortType::Input).port_names(),
            vec!["input1", "input2"]
        );
    }
}
