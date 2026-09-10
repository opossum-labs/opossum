use super::OpticGraph;
use crate::{
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    light::LightFlow,
    nodes::NodeGroup,
    properties::proptype::format_quantity,
};
use petgraph::graph::{EdgeIndex, NodeIndex};
use std::fmt::Write;
use uom::si::length::meter;

impl OpticGraph {
    /// Creates the dot-format string which describes the edge that connects two nodes
    ///
    /// # Parameters:
    /// * `end_node_idx`:         [`NodeIndex`] of the node that should be connected
    /// * `light_port`:           port name that should be connected
    ///
    /// Returns the result of the edge strnig for the dot format
    ///
    /// # Errors
    /// Returns an error if `get_mapped_port_str` or `node_by_idx` fail
    pub fn create_node_edge_str(
        &self,
        end_node_idx: NodeIndex,
        light_port: &str,
    ) -> OpmResult<String> {
        let mut node_ref = self.node_by_idx(end_node_idx)?;
        let node_id = format!("i{}", node_ref.uuid().as_simple());
        if let Some(group_node) = node_ref.as_any_mut().downcast_mut::<NodeGroup>() {
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
            let node_ref = self.node_by_idx(*idx)?;
            let node_name = node_ref.name();
            let inverted = node_ref.inverted();
            let ports = node_ref.ports();
            let uuid = node_ref.node_attr().uuid().as_simple().to_string();
            dot_string += &node_ref.to_dot(&uuid, node_name, inverted, &ports, rankdir)?;
        }
        for edge_idx in self.g.edge_indices() {
            let light: &LightFlow = self.edge_by_idx(edge_idx)?;
            let end_nodes = self
                .g
                .edge_endpoints(edge_idx)
                .ok_or_else(|| OpossumError::Other("could not get edge_endpoints".into()))?;
            let node_id = self.node_by_idx(end_nodes.1)?.uuid();
            let dist = self.distance_from_predecessor(node_id, light.target_port())?;
            let src_edge_str = self.create_node_edge_str(end_nodes.0, light.src_port())?;
            let target_edge_str = self.create_node_edge_str(end_nodes.1, light.target_port())?;

            let _ = writeln!(
                dot_string,
                "  {src_edge_str} -> {target_edge_str} [label=\"{}\"]",
                format_quantity(meter, dist)
            );
        }
        dot_string.push_str("}\n");
        Ok(dot_string)
    }
    fn edge_by_idx(&self, idx: EdgeIndex) -> OpmResult<&LightFlow> {
        self.g
            .edge_weight(idx)
            .ok_or_else(|| OpossumError::Other("could not get edge weight".into()))
    }
}
