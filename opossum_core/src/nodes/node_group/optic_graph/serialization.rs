use super::{super::port_map::PortMap, ConnectionInfo, OpticGraph};
use crate::{
    core_optics::{NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    nodes::{NodeGroup, NodeReference},
    properties::Proptype,
};
use log::warn;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Minimal identity read from a node entry that failed to deserialize into a full [`OpticRef`], so
/// [`deserialize_nodes_lossy`] can name what it is skipping instead of dropping it silently. All fields
/// are optional so this still captures whatever it can from a malformed or partially-broken entry;
/// unrecognized fields on the entry (i.e. everything but these three) are ignored, as for any
/// non-`deny_unknown_fields` struct.
#[derive(Deserialize)]
struct NodeIdentity {
    #[serde(default)]
    node_type: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    uuid: Option<Uuid>,
}
impl std::fmt::Display for NodeIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "node_type: {}, name: {}, uuid: {}",
            self.node_type.as_deref().unwrap_or("<unknown>"),
            self.name.as_deref().unwrap_or("<unknown>"),
            self.uuid
                .map_or_else(|| "<unknown>".to_string(), |u| u.to_string()),
        )
    }
}

/// Custom deserialization helper to skip unknown/invalid nodes in a sequence gracefully, logging a
/// warning for every node it skips so the tolerance from issue #1097 doesn't drop nodes silently (see
/// issue #1144, where a `PortMap` deserialization quirk made this happen to entire groups unnoticed).
fn deserialize_nodes_lossy<'de, D>(deserializer: D) -> Result<Vec<OpticRef>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NodeEntry {
        Valid(OpticRef),
        // Falls back to here when the entry doesn't deserialize into a full `OpticRef` (e.g. an unknown
        // node type) but still exposes enough of its identity to name in the warning below.
        Skipped(NodeIdentity),
        // Falls back to here when the entry can't even be read as `NodeIdentity`.
        Unknown(serde::de::IgnoredAny),
    }

    let entries = Vec::<NodeEntry>::deserialize(deserializer)?;
    let mut valid_nodes = Vec::with_capacity(entries.len());
    for entry in entries {
        match entry {
            NodeEntry::Valid(node) => valid_nodes.push(node),
            NodeEntry::Skipped(identity) => {
                warn!("Skipping node that failed to load ({identity}).");
            }
            NodeEntry::Unknown(_) => {
                warn!("Skipping a node entry that could not be parsed at all.");
            }
        }
    }
    Ok(valid_nodes)
}

// This is the simplified serializable version of an OpticGraph.
#[derive(Serialize, Deserialize)]
pub struct SerializableGraph {
    #[serde(deserialize_with = "deserialize_nodes_lossy")]
    nodes: Vec<OpticRef>,
    edges: Vec<ConnectionInfo>,
    #[serde(default, skip_serializing_if = "PortMap::is_empty")]
    input_map: PortMap,
    #[serde(default, skip_serializing_if = "PortMap::is_empty")]
    output_map: PortMap,
}
impl From<OpticGraph> for SerializableGraph {
    fn from(graph: OpticGraph) -> Self {
        Self {
            nodes: graph.g.node_weights().cloned().collect(),
            edges: graph.connections(),
            input_map: graph.input_port_map,
            output_map: graph.output_port_map,
        }
    }
}

impl TryFrom<SerializableGraph> for OpticGraph {
    type Error = OpossumError;

    fn try_from(temp_graph: SerializableGraph) -> Result<Self, Self::Error> {
        let mut g = Self::default();
        for node in temp_graph.nodes {
            g.g.add_node(node);
        }
        let node_indices = g.g.node_indices().collect::<Vec<_>>();
        for idx in node_indices {
            // Tolerant (`strict = false`): a reference nested here may point at a node in an ancestor or
            // sibling branch that isn't built yet (serde builds inner groups before outer ones). Those are
            // resolved once the whole document exists - see `OpticGraph::resolve_all_references`, driven
            // from `NodeGroup::after_deserialization_hook`.
            let (is_ref, target_uuid) = g.g[idx]
                .as_any()
                .downcast_ref::<NodeReference>()
                .map_or_else(
                    || (false, Uuid::nil()),
                    |refr| {
                        let mut target = refr.referenced_uuid();
                        if target.is_nil()
                            && let Ok(Proptype::Uuid(uuid)) = refr.properties().get("reference id")
                        {
                            target = *uuid;
                        }
                        (true, target)
                    },
                );
            if is_ref
                && !target_uuid.is_nil()
                && let Ok((target_node, _)) = g.node_recursive(target_uuid, Uuid::nil())
                && let Some(refr) = g.g[idx].as_any_mut().downcast_mut::<NodeReference>()
            {
                let ref_name = format!("ref ({})", target_node.name());
                refr.assign_reference(&target_node)?;
                refr.node_attr_mut().set_name(&ref_name);
            }
        }
        for edge in &temp_graph.edges {
            // Log a warning and skip connection if node UUIDs or ports are invalid
            if let Err(e) = g.connect_nodes(
                edge.src_id,
                &edge.src_port,
                edge.target_id,
                &edge.target_port,
                edge.distance,
            ) {
                warn!(
                    "Skipping invalid node connection from '{}' ({}) to '{}' ({}): {e}",
                    edge.src_id, edge.src_port, edge.target_id, edge.target_port
                );
            }
        }
        g.input_port_map = temp_graph.input_map;
        g.output_port_map = temp_graph.output_map;
        Ok(g)
    }
}

impl OpticGraph {
    /// Resolves every reference node in this graph and all nested subgroups against `self` as the search
    /// root, erroring on any whose target isn't anywhere in the tree. Run once after the whole document has
    /// been deserialized (see [`NodeGroup::after_deserialization_hook`]): a reference nested in a group can
    /// point at a node in an ancestor or sibling branch that didn't exist yet when the per-group resolver
    /// ran (serde builds inner groups before outer ones), so those were deferred to here.
    ///
    /// # Errors
    ///
    /// Returns an error if a reference's target isn't found anywhere.
    pub(crate) fn resolve_all_references(&mut self) -> OpmResult<()> {
        let mut references = Vec::new();
        self.collect_reference_node_ids(&mut references)?;
        for (ref_uuid, target_uuid) in references {
            let Ok((target_node, _)) = self.node_recursive(target_uuid, Uuid::nil()) else {
                return Err(OpossumError::Other(
                    "reference node found, which does not reference anything".into(),
                ));
            };
            let ref_node = self.node_recursive_mut(ref_uuid)?;
            if let Some(r) = ref_node.as_any_mut().downcast_mut::<NodeReference>() {
                let ref_name = format!("ref ({})", target_node.name());
                r.assign_reference(&target_node)?;
                r.node_attr_mut().set_name(&ref_name);
            }
        }
        Ok(())
    }

    /// Collects the UUID and referenced UUID of every reference node in this graph and, recursively,
    /// in all nested subgroups.
    fn collect_reference_node_ids(&self, out: &mut Vec<(Uuid, Uuid)>) -> OpmResult<()> {
        for node_ref in self.nodes() {
            if let Some(refr) = node_ref.as_any().downcast_ref::<NodeReference>() {
                let mut target_uuid = refr.referenced_uuid();
                if target_uuid.is_nil()
                    && let Ok(Proptype::Uuid(uuid)) = refr.properties().get("reference id")
                {
                    target_uuid = *uuid;
                }
                out.push((node_ref.uuid(), target_uuid));
            } else if let Some(group) = node_ref.as_any().downcast_ref::<NodeGroup>() {
                group.graph().collect_reference_node_ids(out)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{nodes::Dummy, prelude::PortType, utils::test_helper::test_helper::check_logs};

    /// Regression test for issue #1144: an entry in the node list that isn't even shaped like a node
    /// (and therefore can't be identified via `NodeIdentity` either) must still be skipped with a
    /// warning rather than aborting the whole document load.
    #[test]
    fn deserialize_nodes_lossy_warns_on_unparseable_entry() {
        #[derive(Deserialize)]
        struct Nodes {
            #[serde(deserialize_with = "deserialize_nodes_lossy")]
            nodes: Vec<OpticRef>,
        }

        testing_logger::setup();
        let nodes: Nodes = ron::from_str("(nodes: [42])")
            .expect("an unparseable entry must be skipped, not error");
        assert!(nodes.nodes.is_empty());
        check_logs(
            log::Level::Warn,
            vec!["Skipping a node entry that could not be parsed at all."],
        );
    }

    #[test]
    fn serialize_deserialize() {
        let mut graph = OpticGraph::default();
        let i_d1 = graph.add_node(Dummy::default()).unwrap();
        let i_d2 = graph.add_node(Dummy::default()).unwrap();
        graph
            .map_port(i_d1, &PortType::Input, "input_1", "input_1")
            .unwrap();
        graph
            .map_port(i_d2, &PortType::Input, "input_1", "input_2")
            .unwrap();
        let mut port_names = graph.port_map(&PortType::Input).port_names();
        port_names.sort();
        assert_eq!(port_names, vec!["input_1", "input_2"]);
        let serialized =
            ron::ser::to_string_pretty(&graph, ron::ser::PrettyConfig::new().new_line("\n"))
                .unwrap();
        let deserialized: OpticGraph = ron::from_str(&serialized).unwrap();
        let mut port_names = deserialized.port_map(&PortType::Input).port_names();
        port_names.sort();
        assert_eq!(port_names, vec!["input_1", "input_2"]);
    }

    /// Regression test for a reference surviving a save/reload once its target lives inside a nested group -
    /// the state produced when a referenced node is grouped (see the convert/move relocation fix). Reference
    /// resolution on load used to be single-level (`OpticGraph::node`), so a reference pointing into a
    /// subgroup failed to reload with "reference node found, which does not reference anything"; it now
    /// resolves recursively. Builds `graph { ref -> A, G { A } }`, round-trips it through RON, and asserts
    /// the load succeeds and the reloaded reference still resolves to A (non-empty mirrored ports).
    #[test]
    fn deserialize_reference_into_nested_group() {
        use crate::nodes::{NodeGroup, NodeReference};

        let mut graph = OpticGraph::default();
        let mut g = NodeGroup::new("G");
        let a_id = g.add_node(Dummy::default()).unwrap();
        let a_ref = g.node_recursive(a_id).unwrap().0;
        let r_id = graph
            .add_node(NodeReference::from_node(&a_ref).unwrap())
            .unwrap();
        graph.add_node(g).unwrap();

        let serialized =
            ron::ser::to_string_pretty(&graph, ron::ser::PrettyConfig::new().new_line("\n"))
                .unwrap();
        // Before the fix this errored: A lives inside G, which the single-level lookup missed.
        let deserialized: OpticGraph =
            ron::from_str(&serialized).expect("a reference into a nested group must reload");

        let ref_node = deserialized.node(r_id).unwrap();
        let ports = ref_node.ports();
        assert!(
            !ports.names(&PortType::Output).is_empty(),
            "the reloaded reference must resolve to A (non-empty mirrored ports)"
        );
    }

    /// Regression test for issue #1144: a nested group with mapped ports must survive a round-trip
    /// through RON. Every node entry is deserialized through `deserialize_nodes_lossy`'s
    /// `#[serde(untagged)]` enum, which buffers the entry into serde's generic `Content` representation
    /// first; RON cannot distinguish the group's `input_map`/`output_map` (a `PortMap` newtype struct)
    /// from a one-element tuple while buffering, which used to make `PortMap` fail to reconstruct and -
    /// because `untagged` swallows that failure - silently dropped the *entire* group. See the doc
    /// comment on `PortMap`'s `Deserialize` impl in `port_map.rs` for the full explanation. Builds
    /// `graph { G { d1 -> d2 } }` with G's ports mapped to the outside, round-trips it through RON, and
    /// asserts G and its port mapping are still there afterwards.
    #[test]
    fn deserialize_nested_group_with_mapped_ports() {
        use crate::nodes::NodeGroup;

        let mut g = NodeGroup::new("G");
        let d1 = g.add_node(Dummy::default()).unwrap();
        let d2 = g.add_node(Dummy::default()).unwrap();
        g.map_input_port(d1, "input_1", "input_1").unwrap();
        g.map_output_port(d2, "output_1", "output_1").unwrap();

        let mut graph = OpticGraph::default();
        graph.add_node(g).unwrap();

        let serialized =
            ron::ser::to_string_pretty(&graph, ron::ser::PrettyConfig::new().new_line("\n"))
                .unwrap();
        // Before the fix this silently dropped the group: `deserialized.nodes()` came back empty.
        let deserialized: OpticGraph =
            ron::from_str(&serialized).expect("a nested group with mapped ports must reload");

        assert_eq!(
            deserialized.nodes().len(),
            1,
            "the nested group must survive the round-trip"
        );
        let (input_names, output_names) = {
            let group_ref = &deserialized.nodes()[0];
            let group = group_ref.as_any().downcast_ref::<NodeGroup>().unwrap();
            (
                group.graph().port_map(&PortType::Input).port_names(),
                group.graph().port_map(&PortType::Output).port_names(),
            )
        };
        assert_eq!(input_names, vec!["input_1".to_string()]);
        assert_eq!(output_names, vec!["output_1".to_string()]);
    }
}
