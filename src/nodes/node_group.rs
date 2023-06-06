use crate::optic_node::{OpticNode, Optical};
use crate::error::OpossumError;
use petgraph::algo::*;
use petgraph::prelude::{DiGraph, EdgeIndex, NodeIndex};

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Default)]
pub struct NodeGroup {
    g: DiGraph<OpticNode, ()>,
}

impl NodeGroup {
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a given [`OpticNode`] to the graph of this [`NodeGroup`].
    ///
    /// This command just adds an [`OpticNode`] but does not connect it to existing nodes in the (sub-)graph. The given node is
    /// consumed (owned) by the [`NodeGroup`].
    pub fn add_node(&mut self, node: OpticNode) -> NodeIndex {
        self.g.add_node(node)
    }
    /// Connect (already existing) nodes denoted by the respective `NodeIndex`.
    ///
    /// Both node indices must exist. Otherwise an [`OpticSceneryError`] is returned. In addition, connections are
    /// rejected and an [`OpticSceneryError`] is returned, if the graph would form a cycle (loop in the graph).
    pub fn connect_nodes(
      &mut self,
      src_node: NodeIndex,
      target_node: NodeIndex,
  ) -> Result<EdgeIndex> {
      if self.g.node_weight(src_node).is_none() {
          return Err(OpossumError);
      }
      if self.g.node_weight(target_node).is_none() {
          return Err(OpossumError);
      }
      let edge_index = self.g.add_edge(src_node, target_node, ());
      if is_cyclic_directed(&self.g) {
          self.g.remove_edge(edge_index);
          return Err(OpossumError);
      }
      Ok(edge_index)
  }
}

impl Optical for NodeGroup {
    fn node_type(&self) -> &str {
        "group"
    }
}
