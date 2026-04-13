use super::{super::port_map::PortMap, serialization::SerializableGraph};
use crate::{
    core_optics::{OpticRef, SceneryResources},
    light::LightFlow,
    prelude::PortType,
};
use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use uom::si::f64::Length;
use uuid::Uuid;

/// Represents a connection (edge) between two optical nodes.
///
/// Contains all information necessary to identify the source and target nodes
/// and their ports, along with the distance of the connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    /// UUID of the source node.
    pub src_id: Uuid,
    /// Name of the output port on the source node.
    pub src_port: String,
    /// UUID of the target node.
    pub target_id: Uuid,
    /// Name of the input port on the target node.
    pub target_port: String,
    /// Physical distance between source and target for optical calculations.
    pub distance: Length,
}

impl ConnectionInfo {
    /// Swap the source and target of this connection.
    ///
    /// After calling `invert`, the original `src_id`/`src_port` become the `target_id`/`target_port`,
    /// and vice versa. The `distance` remains unchanged.
    pub fn invert(&mut self) {
        let src_id_buf = self.src_id;
        let src_port_buf = self.src_port.clone();
        self.src_id = self.target_id;
        self.src_port = self.target_port.clone();
        self.target_id = src_id_buf;
        self.target_port = src_port_buf;
    }
}

/// Data structure representing an optical graph
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(try_from = "SerializableGraph", into = "SerializableGraph")]
pub struct OpticGraph {
    pub(super) g: DiGraph<OpticRef, LightFlow>, // pub(super) makes it visible to other modules in optic_graph
    pub(super) input_port_map: PortMap,
    pub(super) output_port_map: PortMap,
    is_inverted: bool,
    external_distances: BTreeMap<String, Length>,
    global_confg: Option<Arc<Mutex<SceneryResources>>>,
}

impl OpticGraph {
    /// Returns `true` if the graph is inverted.
    #[must_use]
    pub const fn is_inverted(&self) -> bool {
        self.is_inverted
    }
    /// Sets the is inverted of this [`OpticGraph`].
    pub const fn set_is_inverted(&mut self, is_inverted: bool) {
        self.is_inverted = is_inverted;
    }
    /// Returns the global config of this [`OpticGraph`].
    #[must_use]
    pub fn global_confg(&self) -> Option<Arc<Mutex<SceneryResources>>> {
        self.global_confg.clone()
    }
    /// Returns a reference to the input port map of this [`OpticGraph`].
    #[must_use]
    pub const fn port_map(&self, port_type: &PortType) -> &PortMap {
        match port_type {
            PortType::Input => &self.input_port_map,
            PortType::Output => &self.output_port_map,
        }
    }
    /// Sets the external distances of this [`OpticGraph`].
    pub fn set_external_distances(&mut self, external_distances: BTreeMap<String, Length>) {
        self.external_distances = external_distances;
    }
    /// Returns a reference to the external distances of this [`OpticGraph`].
    #[must_use]
    pub fn external_distances(&self) -> &BTreeMap<String, Length> {
        &self.external_distances
    }
    /// Update reference to global config for each node in this [`OpticGraph`].
    /// This function is needed after deserialization.
    pub fn update_global_config(&mut self, global_conf: &Option<Arc<Mutex<SceneryResources>>>) {
        for node in self.g.node_weights_mut() {
            node.update_global_config(global_conf.clone());
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn default() {
        let graph = OpticGraph::default();
        assert_eq!(graph.is_inverted, false);
        assert_eq!(graph.g.node_count(), 0)
    }
}
