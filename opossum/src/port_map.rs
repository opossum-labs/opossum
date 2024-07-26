#![warn(missing_docs)]
use itertools::Itertools;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Mapping of the graph's internal `OpticPorts` to externally visible ports.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PortMap(BTreeMap<String, (NodeIndex, String)>);

impl PortMap {
    /// Returns the port names of this [`PortMap`].
    #[must_use]
    pub fn port_names(&self) -> Vec<String> {
        self.0.iter().map(|p| p.0.clone()).collect_vec()
    }
    /// Get the internal node port info for the given external port name.
    #[must_use]
    pub fn get(&self, port_name: &str) -> Option<&(NodeIndex, String)> {
        self.0.get(port_name)
    }
    /// Return the name of the external port name for a given combination of internal [`NodeIndex`] and internal port name.
    ///
    /// This performs a backward search of this [`PortMap`]. This function returns `None` if the given index / port name combintation
    /// was not found.
    #[must_use]
    pub fn external_port_name(&self, idx: NodeIndex, internal_port_name: &str) -> Option<String> {
        let p = self
            .0
            .iter()
            .find(|p| p.1 .0 == idx && p.1 .1 == internal_port_name);
        p.map(|p| p.0.to_string())
    }
    /// Remove a port mapping for the given combination of internal [`NodeIndex`] and internal port name.
    /// If the combination is not found, the [`PortMap`] is unmodified.
    pub fn remove_mapping(&mut self, idx: NodeIndex, internal_port_name: &str) {
        let in_map = self.0.clone();
        let mapping = in_map
            .iter()
            .find(|m| m.1 .0 == idx && m.1 .1 == internal_port_name);
        if let Some(input) = mapping {
            self.0.remove(input.0);
        }
    }
    /// Add a new mapping to this [`PortMap`].
    ///
    /// This function adds a new port mapping to this [`PortMap`] by assigning an external port name to an
    /// internal node index and its respective internal port name
    pub fn add(&mut self, external_name: &str, node_idx: NodeIndex, internal_name: &str) {
        self.0.insert(
            external_name.to_string(),
            (node_idx, internal_name.to_string()),
        );
    }
    /// Check if this [`PortMap`] contains the given external port name.
    pub fn contains_external_name(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
    /// Check if this [`PortMap`] contains the given node.
    pub fn contains_node(&self, node_idx: NodeIndex) -> bool {
        self.0.iter().any(|p| p.1 .0 == node_idx)
    }
    /// Return a vector of port (external -> internal) port assignments for the given node.
    pub fn assigned_ports_for_node(&self, node_idx: NodeIndex) -> Vec<(String, String)> {
        self.0
            .iter()
            .filter(|p| p.1 .0 == node_idx)
            .map(|p| (p.0.to_string(), p.1 .1.to_string()))
            .collect()
    }
    /// Return the number of entries in the port map.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}
