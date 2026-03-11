use super::OpticGraph;
use crate::{
    error::{OpmResult, OpossumError}, light_flow::LightFlow, properties::proptype::format_quantity, utils::LockExt
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
    pub fn create_node_edge_str(&self, end_node_idx: NodeIndex, light_port: &str) -> OpmResult<String> {
        let node_id = format!("i{}", self.node_by_idx(end_node_idx)?.uuid().as_simple());
        let node_ref = self.node_by_idx(end_node_idx)?;
        let mut node = node_ref.optical_ref.lock_opm()?;
        if let Ok(group_node) = node.as_group_mut() {
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
            let node = node_ref.optical_ref.lock_opm()?;
            let node_name = node.name();
            let inverted = node.inverted();
            let ports = node.ports();
            let uuid = node.node_attr().uuid().as_simple().to_string();
            dot_string += &node.to_dot(&uuid, &node_name, inverted, &ports, rankdir)?;
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
