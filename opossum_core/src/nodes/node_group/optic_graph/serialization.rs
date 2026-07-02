use super::{super::port_map::PortMap, ConnectionInfo, OpticGraph};
use crate::{
    core_optics::{OpticRef, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    prelude::OpticNode,
    properties::Proptype,
    utils::LockExt,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// This is the simplified serializable version of an OpticGraph.
#[derive(Serialize, Deserialize)]
pub struct SerializableGraph {
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
            assign_reference_to_ref_node(node_ref, &g)?;
        }
        for edge in &temp_graph.edges {
            g.connect_nodes(
                edge.src_id,
                &edge.src_port,
                edge.target_id,
                &edge.target_port,
                edge.distance,
            )?;
        }
        g.input_port_map = temp_graph.input_map;
        g.output_port_map = temp_graph.output_map;
        Ok(g)
    }
}

fn assign_reference_to_ref_node(node_ref: &OpticRef, graph: &OpticGraph) -> OpmResult<()> {
    if let Ok(ref_node) = node_ref.optical_ref.lock_opm()?.as_refnode_mut() {
        // if Ok, the node was indeed a reference node
        let node_props = ref_node.properties().clone();
        let uuid = if let Proptype::Uuid(uuid) = node_props.get("reference id").unwrap() {
            *uuid
        } else {
            Uuid::nil()
        };
        let Ok(reference_node) = graph.node(uuid) else {
            return Err(OpossumError::Other(
                "reference node found, which does not reference anything".into(),
            ));
        };
        let ref_name = format!("ref ({})", reference_node.optical_ref.lock_opm()?.name());
        ref_node.assign_reference(&reference_node)?;
        ref_node.node_attr_mut().set_name(&ref_name);
    }
    Ok(())
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
}
