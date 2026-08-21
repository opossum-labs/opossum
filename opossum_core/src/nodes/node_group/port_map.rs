#![warn(missing_docs)]
//! # Port map for node groups
//!
//! The `PortMap` struct represents a mapping between externally visible port names and internal node-port pairs within a [`NodeGroup`](super::NodeGroup). It allows to associate an external port name (e.g., `input_1`) with a specific internal port name on a specific node (identified by a [`Uuid`]) within the optical graph of a node group.
use crate::error::{OpmResult, OpossumError};
use serde::{
    Deserialize, Serialize,
    de::{self, Deserializer, MapAccess, SeqAccess, Visitor},
};
use std::collections::HashMap;
use std::fmt;
use utoipa::ToSchema;
use uuid::Uuid;
/// Represents a mapping between externally visible port names and internal node-port pairs.
///
/// The `PortMap` stores associations where an external port name (e.g., `input_1`)
/// maps to a specific internal port name on a specific node (identified by a [`Uuid`])
/// within a the optical graph.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq, ToSchema)]
pub struct PortMap(HashMap<String, (Uuid, String)>);

// Hand-written `Deserialize` for `PortMap` - do not replace this with `#[derive(Deserialize)]`.
//
// `PortMap` is the only newtype struct in the `.opm` format. When a `PortMap` field is deserialized
// through serde's `#[serde(untagged)]` machinery (as node entries are - see `deserialize_nodes_lossy` in
// `optic_graph/serialization.rs`), serde first buffers the whole node into its generic `Content`
// representation via `deserialize_any`. RON's `deserialize_any` cannot distinguish a newtype struct
// `PortMap(...)` from a one-element tuple purely from the token stream `({...})`, so it buffers the value
// as `Content::Seq([Content::Map(...)])` instead of `Content::Newtype(Content::Map(...))`. A derived
// `Deserialize` only accepts a bare map there and fails - and because `untagged` swallows that error and
// falls back to skipping the entry, the *entire* node the `PortMap` lives on (i.e. any group with mapped
// ports) silently disappeared (see issue #1144). This impl accepts both the plain-map shape (produced by
// deserializing directly from RON/JSON/...) and the buffered one-element-sequence-of-map shape (produced
// when routed through `Content`), so a `PortMap` survives either path.
impl<'de> Deserialize<'de> for PortMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_newtype_struct("PortMap", PortMapVisitor)
    }
}

struct PortMapVisitor;

impl<'de> Visitor<'de> for PortMapVisitor {
    type Value = PortMap;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .write_str("a port map, either as a bare map or a one-element sequence wrapping one")
    }

    // What a deserializer that reports the newtype wrapper faithfully calls (RON when reading the
    // original, unbuffered token stream; the `Content` type when it did buffer a genuine
    // `Content::Newtype`). Re-dispatch through `deserialize_any` so `visit_map`/`visit_seq` below handle
    // whichever shape the inner value turns out to be.
    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    // The plain-map shape.
    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        HashMap::deserialize(de::value::MapAccessDeserializer::new(map)).map(PortMap)
    }

    // The buffered one-element-sequence shape; see the doc comment on the `Deserialize` impl above.
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let map = seq
            .next_element::<HashMap<String, (Uuid, String)>>()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        Ok(PortMap(map))
    }
}

impl PortMap {
    /// Add a new mapping to this [`PortMap`].
    ///
    /// This function adds a new port mapping to this [`PortMap`] by assigning an external port name to an
    /// internal node index and its respective internal port name
    ///
    /// # Errors
    /// Returns an error if either the internal or the extrnal name is empty
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
    /// Remove a port mapping for the given combination of internal [`Uuid`] and internal port name.
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

    /// Remove a port mapping for the given combination of internal [`Uuid`] and internal port name.
    /// Returns `true`, if successful. If the combination is not found, the [`PortMap`] is unmodified and `false` is returned.
    pub fn remove_key(&mut self, key: &str) -> bool {
        self.0.remove(key).is_some()
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
    /// Remove all port mappings from this [`PortMap`].
    pub fn clear(&mut self) {
        self.0.clear();
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
    /// Return the name of the external port name for a given combination of internal [`Uuid`] and internal port name.
    ///
    /// This performs a backward search of this [`PortMap`]. This function returns `None` if the given index / port name combination
    /// was not found.
    #[must_use]
    pub fn external_port_name(&self, node_id: Uuid, internal_port_name: &str) -> Option<String> {
        let p = self
            .0
            .iter()
            .find(|p| p.1.0 == node_id && p.1.1 == internal_port_name);
        p.map(|p| p.0.clone())
    }
    /// Check if this [`PortMap`] contains the given external port name.
    #[must_use]
    pub fn contains_external_name(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
    /// Check if this [`PortMap`] contains the given node.
    #[must_use]
    pub fn contains_node(&self, node_id: Uuid) -> bool {
        self.0.iter().any(|p| p.1.0 == node_id)
    }

    /// Retrieve the external port name of a mapped port from the id of the internal node and the name of the internal port
    #[must_use]
    pub fn external_port_of_mapped_port(&self, node_id: Uuid, port_name: &str) -> Option<String> {
        self.0
            .iter()
            .find(|(_, (id, name))| *id == node_id && name == port_name)
            .map(|(name, _)| name.clone())
    }
    /// Check if a port of an internal node wit specific id and port name is mapped
    #[must_use]
    pub fn contains_port_of_node(&self, node_id: Uuid, port_name: &str) -> bool {
        self.0
            .iter()
            .any(|(_, (id, name))| *id == node_id && name == port_name)
    }
    /// Return a vector of port (external -> internal) port assignments for the given node.
    #[must_use]
    pub fn assigned_ports_for_node(&self, node_id: Uuid) -> Vec<(String, String)> {
        self.0
            .iter()
            .filter(|p| p.1.0 == node_id)
            .map(|p| (p.0.clone(), p.1.1.clone()))
            .collect()
    }
    /// Returns the total number of external port mappings in this [`PortMap`].
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Returns `true` if the [`PortMap`] is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Returns an iterator of this [`PortMap`]
    #[must_use]
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, (Uuid, String)> {
        self.0.iter()
    }
}
impl<'a> IntoIterator for &'a PortMap {
    type Item = (&'a String, &'a (Uuid, String));
    type IntoIter = std::collections::hash_map::Iter<'a, String, (Uuid, String)>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deserialize_direct_map() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        port_map.add("external1", Uuid::new_v4(), "internal1")?;
        let serialized = ron::ser::to_string(&port_map).unwrap();
        let deserialized: PortMap = ron::from_str(&serialized).unwrap();
        assert_eq!(deserialized, port_map);
        Ok(())
    }
    /// Regression test for issue #1144: `PortMap` must also survive being routed through serde's generic
    /// `Content` buffering, which is what happens whenever it is nested inside a `#[serde(untagged)]`
    /// enum (as it is via `NodeGroup`'s node entries during `.opm` loading, see
    /// `deserialize_nodes_lossy` in `optic_graph/serialization.rs`). RON cannot tell a newtype struct
    /// apart from a one-element tuple while buffering, which used to make this round-trip fail while
    /// the direct one above kept passing - see the doc comment on `PortMap`'s `Deserialize` impl.
    #[test]
    fn deserialize_through_untagged_buffering() -> OpmResult<()> {
        #[derive(serde::Serialize, serde::Deserialize)]
        #[serde(untagged)]
        enum Wrapper {
            Map(PortMap),
        }

        let mut port_map = PortMap::default();
        port_map.add("external1", Uuid::new_v4(), "internal1")?;

        let serialized = ron::ser::to_string(&port_map).unwrap();
        let Wrapper::Map(deserialized) = ron::from_str(&serialized).unwrap();
        assert_eq!(deserialized, port_map);
        Ok(())
    }
    #[test]
    fn add() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        assert!(port_map.0.is_empty());
        assert!(port_map.add("", Uuid::new_v4(), "internal1").is_err());
        assert!(port_map.add("external1", Uuid::new_v4(), "").is_err());
        port_map.add("external1", Uuid::new_v4(), "internal1")?;
        assert_eq!(port_map.0.len(), 1);
        Ok(())
    }
    #[test]
    fn remove() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        assert_eq!(port_map.remove(Uuid::new_v4(), "internal1"), false);
        let uuid = Uuid::new_v4();
        port_map.add("external1", uuid, "internal1")?;
        assert_eq!(port_map.remove(Uuid::nil(), "internal1"), false);
        assert_eq!(port_map.remove(uuid, "internal2"), false);
        assert_eq!(port_map.remove(uuid, "internal1"), true);
        assert!(port_map.0.is_empty());
        Ok(())
    }
    #[test]
    fn remove_all_from_uuid() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        assert_eq!(port_map.remove_all_from_uuid(Uuid::new_v4()), false);
        let uuid1 = Uuid::new_v4();
        port_map.add("external1", uuid1, "internal1")?;
        port_map.add("external2", uuid1, "internal2")?;
        let uuid2 = Uuid::new_v4();
        port_map.add("external3", uuid2, "internal1")?;
        port_map.add("external4", uuid2, "internal2")?;
        port_map.add("external5", uuid2, "internal3")?;
        assert_eq!(port_map.remove_all_from_uuid(Uuid::nil()), false);
        assert_eq!(port_map.remove_all_from_uuid(uuid1), true);
        assert_eq!(port_map.0.len(), 3);
        assert_eq!(port_map.remove_all_from_uuid(uuid2), true);
        assert!(port_map.0.is_empty());
        Ok(())
    }
    #[test]
    fn clear() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        port_map.add("external1", Uuid::new_v4(), "internal1")?;
        port_map.add("external2", Uuid::new_v4(), "internal2")?;
        assert_eq!(port_map.0.len(), 2);
        port_map.clear();
        assert!(port_map.0.is_empty());
        Ok(())
    }
    #[test]
    fn port_names() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        port_map.add("external1", Uuid::new_v4(), "internal1")?;
        port_map.add("external2", Uuid::new_v4(), "internal2")?;
        let mut port_names = port_map.port_names();
        port_names.sort();
        assert_eq!(port_names, vec!["external1", "external2"]);
        Ok(())
    }
    #[test]
    fn get() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        let node_id = Uuid::new_v4();
        port_map.add("external1", node_id, "internal1")?;
        assert_eq!(
            port_map.get("external1"),
            Some(&(node_id, "internal1".to_string()))
        );
        assert_eq!(port_map.get("external2"), None);
        Ok(())
    }
    #[test]
    fn external_port_name() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        let node_id = Uuid::new_v4();
        port_map.add("external1", node_id, "internal1")?;
        assert_eq!(
            port_map.external_port_name(node_id, "internal1"),
            Some("external1".to_string())
        );
        assert_eq!(port_map.external_port_name(node_id, "internal2"), None);
        Ok(())
    }
    #[test]
    fn contains_external_name() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        port_map.add("external1", Uuid::new_v4(), "internal1")?;
        port_map.add("external2", Uuid::new_v4(), "internal2")?;
        assert!(port_map.contains_external_name("external1"));
        assert!(port_map.contains_external_name("external2"));
        assert!(!port_map.contains_external_name("external3"));
        Ok(())
    }
    #[test]
    fn contains_node() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();
        port_map.add("external1", node_id1, "internal1")?;
        port_map.add("external2", node_id2, "internal2")?;
        assert!(port_map.contains_node(node_id1));
        assert!(port_map.contains_node(node_id2));
        assert!(!port_map.contains_node(Uuid::nil()));
        Ok(())
    }
    #[test]
    fn assigned_ports_for_node() -> OpmResult<()> {
        let mut port_map = PortMap::default();
        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();
        port_map.add("external1", node_id1, "internal1")?;
        port_map.add("external2", node_id1, "internal2")?;
        port_map.add("external3", node_id2, "internal2")?;
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
        Ok(())
    }
}
