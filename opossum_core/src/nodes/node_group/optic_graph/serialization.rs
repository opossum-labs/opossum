use super::{super::port_map::PortMap, ConnectionInfo, OpticGraph};
use crate::{
    core_optics::{NodeAttrExt, OpticRef, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    nodes::{NodeGroup, NodeReference},
    properties::Proptype,
    utils::LockExt,
};
use log::warn;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Custom deserialization helper to skip unknown/invalid nodes in a sequence gracefully.
fn deserialize_nodes_lossy<'de, D>(deserializer: D) -> Result<Vec<OpticRef>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NodeEntry {
        Valid(OpticRef),
        Unknown(serde::de::IgnoredAny),
    }

    let entries = Vec::<NodeEntry>::deserialize(deserializer)?;
    let mut valid_nodes = Vec::with_capacity(entries.len());
    for entry in entries {
        if let NodeEntry::Valid(node) = entry {
            valid_nodes.push(node);
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
        for node in &temp_graph.nodes {
            g.g.add_node(node.clone());
        }
        for node_ref in &temp_graph.nodes {
            // Tolerant (`strict = false`): a reference nested here may point at a node in an ancestor or
            // sibling branch that isn't built yet (serde builds inner groups before outer ones). Those are
            // resolved once the whole document exists - see `OpticGraph::resolve_all_references`, driven
            // from `NodeGroup::after_deserialization_hook`.
            assign_reference_to_ref_node(node_ref, &g, false)?;
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

/// Resolves the reference `node_ref` (if it is one) against `graph`, pointing it at its target and
/// refreshing its `ref (...)` name.
///
/// `graph.node_recursive` searches `graph` and all its nested subgroups, so passing the *root* graph
/// resolves a reference to a target anywhere in the scenery (up, down, or sideways). `strict` controls the
/// not-found behaviour: `false` (per-group, mid-deserialization) skips silently, because the target may
/// live in an ancestor/sibling not built yet and will be resolved by the whole-scenery pass; `true`
/// (that whole-scenery pass) errors, since by then a missing target is genuinely dangling.
///
/// # Errors
///
/// Returns an error if a lock can't be acquired, or (only when `strict`) the target isn't found anywhere.
fn assign_reference_to_ref_node(
    node_ref: &OpticRef,
    graph: &OpticGraph,
    strict: bool,
) -> OpmResult<()> {
    // Read the referenced uuid, then release the lock *before* searching the graph below - the recursive
    // lookup locks every node it visits (to descend into groups), which would deadlock against a lock held
    // on this same reference node. The lock guard is a temporary here (not a named binding), so it is
    // dropped at the end of this `match`, before the search.
    let referenced_uuid = match node_ref
        .optical_ref
        .lock_opm()?
        .as_any()
        .downcast_ref::<NodeReference>()
    {
        Some(ref_node) => match ref_node.properties().get("reference id") {
            Ok(Proptype::Uuid(uuid)) => *uuid,
            _ => Uuid::nil(),
        },
        None => return Ok(()), // not a reference node - nothing to assign
    };
    // `node_recursive` tries this level first (so intra-group references are unchanged), then descends into
    // subgroups. The group-id it also returns is unused here.
    let Ok((reference_node, _)) = graph.node_recursive(referenced_uuid, Uuid::nil()) else {
        if strict {
            return Err(OpossumError::Other(
                "reference node found, which does not reference anything".into(),
            ));
        }
        return Ok(()); // deferred to the whole-scenery pass (see `resolve_all_references`)
    };
    let ref_name = format!("ref ({})", reference_node.optical_ref.lock_opm()?.name());
    if let Some(ref_node) = node_ref
        .optical_ref
        .lock_opm()?
        .as_any_mut()
        .downcast_mut::<NodeReference>()
    {
        ref_node.assign_reference(&reference_node)?;
        ref_node.node_attr_mut().set_name(&ref_name);
    }
    Ok(())
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
    /// Returns an error if a contained node can't be locked, or a reference's target isn't found anywhere.
    pub(crate) fn resolve_all_references(&self) -> OpmResult<()> {
        let mut references = Vec::new();
        self.collect_reference_nodes(&mut references)?;
        // Resolve after collection, so no per-node lock is held across `node_recursive`'s own whole-tree
        // walk (which would deadlock against it).
        for reference in &references {
            assign_reference_to_ref_node(reference, self, true)?;
        }
        Ok(())
    }

    /// Collects every reference node in this graph and, recursively, in all nested subgroups. Locks are
    /// only held briefly per node (never across the resolution above), so the subsequent whole-tree
    /// searches can't deadlock against a lock held here.
    ///
    /// # Errors
    ///
    /// Returns an error if a contained node can't be locked.
    // The single `node` guard is used by both branches; the group branch must keep it locked across its
    // recursion (which borrows the locked node), so it can't be tightened as the lint wants.
    #[allow(clippy::significant_drop_tightening)]
    fn collect_reference_nodes(&self, out: &mut Vec<OpticRef>) -> OpmResult<()> {
        for node_ref in self.nodes() {
            let node = node_ref.optical_ref.lock_opm()?;
            if node.as_any().downcast_ref::<NodeReference>().is_some() {
                out.push(node_ref.clone());
            } else if let Some(group) = node.as_any().downcast_ref::<NodeGroup>() {
                group.graph().collect_reference_nodes(out)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{nodes::Dummy, prelude::PortType};

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
        let ports = ref_node.optical_ref.lock_opm().unwrap().ports();
        assert!(
            !ports.names(&PortType::Output).is_empty(),
            "the reloaded reference must resolve to A (non-empty mirrored ports)"
        );
    }
}
