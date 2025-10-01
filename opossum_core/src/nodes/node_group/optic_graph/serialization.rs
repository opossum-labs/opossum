use super::{ConnectionInfo, OpticGraph};
use crate::{
    error::{OpmResult, OpossumError},
    optic_ref::OpticRef,
    port_map::PortMap,
    prelude::OpticNode,
    properties::Proptype,
};
use serde::{Deserialize, Serialize, de, ser::SerializeStruct};
use uuid::Uuid;

impl Serialize for OpticGraph {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // 1. Dynamically count the number of fields to be serialized.
        //    Start with the fields that are always present.
        let mut field_count = 2; // "nodes" and "edges"
        if self.input_port_map.len() != 0 {
            field_count += 1;
        }
        if self.output_port_map.len() != 0 {
            field_count += 1;
        }

        // 2. Start serialization with the correct field count.
        let mut graph = serializer.serialize_struct("graph", field_count)?;

        // 3. Serialize mandatory fields.
        let nodes = self.g.node_weights().cloned().collect::<Vec<OpticRef>>();
        graph.serialize_field("nodes", &nodes)?;

        // You can reuse your existing `connections()` method here for cleaner code.
        let connections = self.connections();
        graph.serialize_field("edges", &connections)?;

        // 4. Conditionally serialize the port maps.
        if self.input_port_map.len() != 0 {
            graph.serialize_field("input_map", &self.input_port_map)?;
        }
        if self.output_port_map.len() != 0 {
            graph.serialize_field("output_map", &self.output_port_map)?;
        }

        graph.end()
    }
}

impl<'de> Deserialize<'de> for OpticGraph {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Define a helper struct that matches the serialized format.
        #[derive(Deserialize)]
        struct SerializableGraph {
            nodes: Vec<OpticRef>,
            edges: Vec<ConnectionInfo>,
            #[serde(default)]
            input_map: PortMap,
            #[serde(default)]
            output_map: PortMap,
        }
        let temp_graph = SerializableGraph::deserialize(deserializer)?;

        let mut g = Self::default();

        // 1. Add all nodes to the graph.
        for node in &temp_graph.nodes {
            g.g.add_node(node.clone());
        }
        // 2. Assign references for any reference nodes. This must be done after all
        //    nodes are already in the graph, so that the referenced node can be found.
        for node_ref in &temp_graph.nodes {
            assign_reference_to_ref_node(node_ref, &g)
                .map_err(|e| de::Error::custom(e.to_string()))?;
        }
        // 3. Re-create all the connections (edges) between the nodes.
        for edge in &temp_graph.edges {
            g.connect_nodes(
                edge.src_id,
                &edge.src_port,
                edge.target_id,
                &edge.target_port,
                edge.distance,
            )
            .map_err(|e| de::Error::custom(format!("connecting OpticGraph nodes failed: {e}")))?;
        }
        // 4. Assign the port maps (might be empty (default value))
        g.input_port_map = temp_graph.input_map;
        g.output_port_map = temp_graph.output_map;
        Ok(g)
    }
}

fn assign_reference_to_ref_node(node_ref: &OpticRef, graph: &OpticGraph) -> OpmResult<()> {
    if let Ok(ref_node) = node_ref
        .optical_ref
        .lock()
        .expect("Mutex lock failed")
        .as_refnode_mut()
    {
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
        let ref_name = format!(
            "ref ({})",
            reference_node
                .optical_ref
                .lock()
                .expect("Mutex lock failed")
                .name()
        );
        ref_node.assign_reference(&reference_node);
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
