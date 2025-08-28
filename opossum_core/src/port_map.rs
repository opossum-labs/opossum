#![warn(missing_docs)]
use crate::error::{OpmResult, OpossumError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
/// Represents a mapping between externally visible port names and internal node-port pairs.
///
/// The `PortMap` stores associations where an external port name (e.g., `input_1`)
/// maps to a specific internal port name on a specific node (identified by a [`Uuid`])
/// within a the optical graph.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PortMap(HashMap<String, (Uuid, String)>);

impl PortMap {
    /// Add a new mapping to this [`PortMap`].
    ///
    /// This function adds a new port mapping to this [`PortMap`] by assigning an external port name to an
    /// internal node index and its respective internal port name
    pub fn add(
        &mut self,
        external_name: &str,
        node_id: Uuid,
        internal_name: &str,
    ) -> OpmResult<()> {
        if external_name.is_empty() || internal_name.is_empty() {
            return Err(OpossumError::OpticPort(
                "internal and external port names must not be empty".into(),
            ));
        }
        self.0.insert(
            external_name.to_string(),
            (node_id, internal_name.to_string()),
        );
        Ok(())
    }
    /// Remove a port mapping for the given combination of internal [`NodeIndex`] and internal port name.
    /// Returns `true`, if successful. If the combination is not found, the [`PortMap`] is unmodified and `false` is returned.
    pub fn remove(&mut self, node_id: Uuid, internal_port_name: &str) -> bool {
        let key_to_remove = self
            .0
            .iter()
            .find(|(_, (current_node_id, current_internal_name))| {
                *current_node_id == node_id && current_internal_name == internal_port_name
            })
            .map(|(external_name, _)| external_name.clone());

        if let Some(key) = key_to_remove {
            self.0.remove(&key).is_some()
        } else {
            false
        }
    }
    /// Remove all port mappings for the node with the given [`Uuid`].
    ///
    /// Returns `true` if elements have been removed and `false` otherwise.
    pub fn remove_all_from_uuid(&mut self, node_id: Uuid) -> bool {
        let len_before = self.0.len();
        self.0.retain(|_, v| v.0 != node_id);
        let len_after = self.0.len();
        len_after < len_before
    }
    /// Returns the port names of this [`PortMap`].
    #[must_use]
    pub fn port_names(&self) -> Vec<String> {
        self.0.iter().map(|p| p.0.clone()).collect::<Vec<String>>()
    }
    /// Get the internal node port info for the given external port name.
    #[must_use]
    pub fn get(&self, port_name: &str) -> Option<&(Uuid, String)> {
        self.0.get(port_name)
    }
    /// Return the name of the external port name for a given combination of internal [`NodeIndex`] and internal port name.
    ///
    /// This performs a backward search of this [`PortMap`]. This function returns `None` if the given index / port name combination
    /// was not found.
    #[must_use]
    pub fn external_port_name(&self, node_id: Uuid, internal_port_name: &str) -> Option<String> {
        let p = self
            .0
            .iter()
            .find(|p| p.1.0 == node_id && p.1.1 == internal_port_name);
        p.map(|p| p.0.to_string())
    }
    /// Check if this [`PortMap`] contains the given external port name.
    pub fn contains_external_name(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
    /// Check if this [`PortMap`] contains the given node.
    pub fn contains_node(&self, node_id: Uuid) -> bool {
        self.0.iter().any(|p| p.1.0 == node_id)
    }
    /// Return a vector of port (external -> internal) port assignments for the given node.
    pub fn assigned_ports_for_node(&self, node_id: Uuid) -> Vec<(String, String)> {
        self.0
            .iter()
            .filter(|p| p.1.0 == node_id)
            .map(|p| (p.0.to_string(), p.1.1.to_string()))
            .collect()
    }
    /// Returns the total number of external port mappings in this [`PortMap`].
    pub fn len(&self) -> usize {
        self.0.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add() {
        let mut port_map = PortMap::default();
        assert!(port_map.0.is_empty());
        assert!(port_map.add("", Uuid::new_v4(), "internal1").is_err());
        assert!(port_map.add("external1", Uuid::new_v4(), "").is_err());
        port_map
            .add("external1", Uuid::new_v4(), "internal1")
            .unwrap();
        assert_eq!(port_map.0.len(), 1);
    }
    #[test]
    fn remove() {
        let mut port_map = PortMap::default();
        assert_eq!(port_map.remove(Uuid::new_v4(), "internal1"), false);
        let uuid = Uuid::new_v4();
        port_map.add("external1", uuid, "internal1").unwrap();
        assert_eq!(port_map.remove(Uuid::nil(), "internal1"), false);
        assert_eq!(port_map.remove(uuid, "internal2"), false);
        assert_eq!(port_map.remove(uuid, "internal1"), true);
        assert!(port_map.0.is_empty());
    }
    #[test]
    fn remove_all_from_uuid() {
        let mut port_map = PortMap::default();
        assert_eq!(port_map.remove_all_from_uuid(Uuid::new_v4()), false);
        let uuid1 = Uuid::new_v4();
        port_map.add("external1", uuid1, "internal1").unwrap();
        port_map.add("external2", uuid1, "internal2").unwrap();
        let uuid2 = Uuid::new_v4();
        port_map.add("external3", uuid2, "internal1").unwrap();
        port_map.add("external4", uuid2, "internal2").unwrap();
        port_map.add("external5", uuid2, "internal3").unwrap();
        assert_eq!(port_map.remove_all_from_uuid(Uuid::nil()), false);
        assert_eq!(port_map.remove_all_from_uuid(uuid1), true);
        assert_eq!(port_map.0.len(), 3);
        assert_eq!(port_map.remove_all_from_uuid(uuid2), true);
        assert!(port_map.0.is_empty());
    }
    #[test]
    fn port_names() {
        let mut port_map = PortMap::default();
        port_map
            .add("external1", Uuid::new_v4(), "internal1")
            .unwrap();
        port_map
            .add("external2", Uuid::new_v4(), "internal2")
            .unwrap();
        let mut port_names = port_map.port_names();
        port_names.sort();
        assert_eq!(port_names, vec!["external1", "external2"]);
    }
    #[test]
    fn get() {
        let mut port_map = PortMap::default();
        let node_id = Uuid::new_v4();
        port_map.add("external1", node_id, "internal1").unwrap();
        assert_eq!(
            port_map.get("external1"),
            Some(&(node_id, "internal1".to_string()))
        );
        assert_eq!(port_map.get("external2"), None);
    }
    #[test]
    fn external_port_name() {
        let mut port_map = PortMap::default();
        let node_id = Uuid::new_v4();
        port_map.add("external1", node_id, "internal1").unwrap();
        assert_eq!(
            port_map.external_port_name(node_id, "internal1"),
            Some("external1".to_string())
        );
        assert_eq!(port_map.external_port_name(node_id, "internal2"), None);
    }
    #[test]
    fn contains_external_name() {
        let mut port_map = PortMap::default();
        port_map
            .add("external1", Uuid::new_v4(), "internal1")
            .unwrap();
        port_map
            .add("external2", Uuid::new_v4(), "internal2")
            .unwrap();
        assert!(port_map.contains_external_name("external1"));
        assert!(port_map.contains_external_name("external2"));
        assert!(!port_map.contains_external_name("external3"));
    }
    #[test]
    fn contains_node() {
        let mut port_map = PortMap::default();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();
        port_map.add("external1", node_id1, "internal1").unwrap();
        port_map.add("external2", node_id2, "internal2").unwrap();
        assert!(port_map.contains_node(node_id1));
        assert!(port_map.contains_node(node_id2));
        assert!(!port_map.contains_node(Uuid::nil()));
    }
    #[test]
    fn assigned_ports_for_node() {
        let mut port_map = PortMap::default();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();
        port_map.add("external1", node_id1, "internal1").unwrap();
        port_map.add("external2", node_id1, "internal2").unwrap();
        port_map.add("external3", node_id2, "internal2").unwrap();
        let mut ports = port_map.assigned_ports_for_node(node_id1);
        ports.sort();
        assert_eq!(ports[0].0, "external1");
        assert_eq!(ports[0].1, "internal1");
        assert_eq!(ports[1].0, "external2");
        assert_eq!(ports[1].1, "internal2");
        let ports = port_map.assigned_ports_for_node(node_id2);
        assert_eq!(ports[0].0, "external3");
        assert_eq!(ports[0].1, "internal2");
        assert!(port_map.assigned_ports_for_node(Uuid::nil()).is_empty());
    }
}
