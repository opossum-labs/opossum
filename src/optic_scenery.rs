use crate::optic_node::OpticNode;
use petgraph::algo::*;
use petgraph::prelude::{DiGraph, EdgeIndex, NodeIndex};

#[derive(Debug, Clone)]
pub struct OpticSceneryError;
type Result<T> = std::result::Result<T, OpticSceneryError>;

/// [`OpticScenery`] represents the overall optical model and additional metatdata. All optical elements ([`OpticNode`]s) have
/// to be added to this structure in order to be considered for an analysis.
#[derive(Default, Debug)]
pub struct OpticScenery {
    g: DiGraph<OpticNode, ()>,
    description: String,
}

impl OpticScenery {
    /// Creates a new (empty) [`OpticScenery`].
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a given [`OpticNode`] to the graph of this [`OpticScenery`].
    ///
    /// This command just adds an [`OpticNode`] but does not connect it to existing nodes in the graph. The given node is
    /// consumed (owned) by the [`OpticScenery`].
    pub fn add_node(&mut self, node: OpticNode) -> NodeIndex {
        self.g.add_node(node)
    }
    /// Connect to (already existing) nodes denoted by the respective `NodeIndex`.
    ///
    /// Both node indices must exist. Otherwise an [`OpticSceneryError`] is returned. In addition, connections are
    /// rejected and an [`OpticSceneryError`] is returned, if the graph would form a cycle (loop in the graph).
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        target_node: NodeIndex,
    ) -> Result<EdgeIndex> {
        if self.g.node_weight(src_node).is_none() {
            return Err(OpticSceneryError);
        }
        if self.g.node_weight(target_node).is_none() {
            return Err(OpticSceneryError);
        }
        let edge_index = self.g.add_edge(src_node, target_node, ());
        if is_cyclic_directed(&self.g) {
            self.g.remove_edge(edge_index);
            return Err(OpticSceneryError);
        }
        Ok(edge_index)
    }
    /// Export the optic graph into the `dot` format to be used in combination with the [`graphviz`](https://graphviz.org/) software.
    pub fn to_dot(&self) -> String {
        let mut dot_string = "digraph {\n".to_owned();
        dot_string.push_str(&format!("  label=\"{}\"\n", self.description));
        for node_idx in self.g.node_indices() {
            let node=self.g.node_weight(node_idx).unwrap();
            dot_string.push_str(&format!("  node_idx_{} ", node_idx.index()));
            dot_string += &node.to_dot();
        }
        for edge in self.g.edge_indices() {
            let end_nodes = self.g.edge_endpoints(edge).unwrap();
            let node1 = self.g.node_weight(end_nodes.0).unwrap();
            let node2 = self.g.node_weight(end_nodes.1).unwrap();
            dot_string.push_str(&format!("  \"{}\" -> \"{}\"\n", node1.name(), node2.name()));
        }
        dot_string += "}";
        dot_string
    }
    /// Analyze this [`OpticScenery`] using a given OpticAnalyzer.
    pub fn analyze(&self) {
        todo!();
    }
    /// Sets the description of this [`OpticScenery`].
    pub fn set_description(&mut self, description: String) {
        self.description = description;
    }
    /// Returns a reference to the description of this [`OpticScenery`].
    pub fn description(&self) -> &str {
        self.description.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::super::nodes::NodeDummy;
    use super::*;
    #[test]
    fn new() {
        let scenery = OpticScenery::new();
        assert_eq!(scenery.description, "".to_owned());
        assert_eq!(scenery.g.edge_count(), 0);
        assert_eq!(scenery.g.node_count(), 0);
    }
    // #[test]
    // fn add_node() {
    //     let mut scenery = OpticScenery::new();
    //     scenery.add_node(OpticNode::new("Test", Box::new(NodeDummy)));
    //     assert_eq!(scenery.g.node_count(), 1);
    // }
    #[test]
    fn connect_nodes_ok() {
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_node(OpticNode::new("Test", Box::new(NodeDummy)));
        let n2 = scenery.add_node(OpticNode::new("Test", Box::new(NodeDummy)));
        assert!(scenery.connect_nodes(n1, n2).is_ok());
        assert_eq!(scenery.g.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_failure() {
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_node(OpticNode::new("Test", Box::new(NodeDummy)));
        let n2 = scenery.add_node(OpticNode::new("Test", Box::new(NodeDummy)));
        assert!(scenery.connect_nodes(n1, NodeIndex::new(5)).is_err());
        assert!(scenery.connect_nodes(NodeIndex::new(5), n2).is_err());
    }
    #[test]
    fn connect_nodes_loop_error() {
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_node(OpticNode::new("Test", Box::new(NodeDummy)));
        let n2 = scenery.add_node(OpticNode::new("Test", Box::new(NodeDummy)));
        assert!(scenery.connect_nodes(n1, n2).is_ok());
        assert!(scenery.connect_nodes(n2, n1).is_err());
        assert_eq!(scenery.g.edge_count(), 1);
    }
    #[test]
    fn to_dot_empty() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.to_dot(), "digraph {\n  label=\"Test\"\n}");
    }
    #[test]
    fn to_dot_with_node() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into());
        scenery.add_node(OpticNode::new("Test", Box::new(NodeDummy)));
        assert_eq!(
            scenery.to_dot(),
            "digraph {\n  label=\"SceneryTest\"\n  \"Test\"\n}"
        );
    }
    #[test]
    fn to_dot_with_edge() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into());
        let n1=scenery.add_node(OpticNode::new("Test1", Box::new(NodeDummy)));
        let n2=scenery.add_node(OpticNode::new("Test2", Box::new(NodeDummy)));
        scenery.connect_nodes(n1,n2);
        assert_eq!(
            scenery.to_dot(),
            "digraph {\n  label=\"SceneryTest\"\n  \"Test1\"\n  \"Test2\"\n  \"Test1\" -> \"Test2\"\n}"
        );
    }
    #[test]
    fn set_description() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.description, "Test")
    }
    #[test]
    fn description() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.description(), "Test")
    }
}
