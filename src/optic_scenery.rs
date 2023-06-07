use crate::error::OpossumError;
use crate::light::Light;
use crate::optic_node::{OpticNode, Optical};
use petgraph::algo::*;
use petgraph::prelude::{DiGraph, EdgeIndex, NodeIndex};

type Result<T> = std::result::Result<T, OpossumError>;

/// [`OpticScenery`] represents the overall optical model and additional metatdata. All optical elements ([`OpticNode`]s) have
/// to be added to this structure in order to be considered for an analysis.
#[derive(Default, Debug)]
pub struct OpticScenery {
    g: DiGraph<OpticNode, Light>,
    description: String,
}

impl OpticScenery {
    /// Creates a new (empty) [`OpticScenery`].
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a given [`OpticNode`] to the graph of this [`OpticScenery`].
    ///
    /// This command just adds an [`OpticNode`] to the graph. It does not connect
    /// it to existing nodes in the graph. The given optical element is consumed (owned) by the [`OpticScenery`].
    pub fn add_node(&mut self, node: OpticNode) -> NodeIndex {
        self.g.add_node(node)
    }
    /// Add a given optical element to the graph of this [`OpticScenery`].
    ///
    /// This command just adds an optical element (a struct implementing the [`Optical`] trait such as `OpticDummy` ) to the graph. It does not connect
    /// it to existing nodes in the graph. The given optical element is consumed (owned) by the [`OpticScenery`]. Internally the corresponding [`OpticNode`] is
    /// automatically generated. It serves as a short-cut to the `add_node` function.
    pub fn add_element<T: Optical + 'static>(&mut self, name: &str, t: T) -> NodeIndex {
        self.g.add_node(OpticNode::new(name, t))
    }
    /// Get reference of [`OpticNode`].
    ///
    /// Get the reference of an previously added [`OpticNode`] denoted by a given `NodeIndex`. This function can be used as input while
    /// constructing a `NodeReference`.
    /// # Panics
    ///
    /// Panics if the given `NodeIndex` is not found in the graph.
    pub fn node(&self, idx: NodeIndex) -> &OpticNode {
        self.g.node_weight(idx).unwrap()
    }
    /// Connect (already existing) nodes denoted by the respective `NodeIndex`.
    ///
    /// Both node indices must exist. Otherwise an [`OpticSceneryError`] is returned. In addition, connections are
    /// rejected and an [`OpticSceneryError`] is returned, if the graph would form a cycle (loop in the graph).
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
    ) -> Result<EdgeIndex> {
        if let Some(source) = self.g.node_weight(src_node) {
            if !source.ports().outputs().contains(&src_port.into()) {
                return Err(OpossumError::OpticScenery(format!(
                    "source node {} does not have a port {}",
                    source.name(),
                    src_port
                )));
            }
        } else {
            return Err(OpossumError::OpticScenery(
                "source node with given index does not exist".into(),
            ));
        }
        if let Some(target) = self.g.node_weight(target_node) {
            if !target.ports().inputs().contains(&target_port.into()) {
                return Err(OpossumError::OpticScenery(format!(
                    "target node {} does not have a port {}",
                    target.name(),
                    target_port
                )));
            }
        } else {
            return Err(OpossumError::OpticScenery(
                "target node with given index does not exist".into(),
            ));
        }
        // TODO: Check if src_node,src_port combination already exists in the graph
        // TODO: Checl if target_not/target_port combination already exists in the graph
        let edge_index = self.g.add_edge(src_node, target_node, Light::new(src_port, target_port));
        if is_cyclic_directed(&self.g) {
            self.g.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(
                "connecting the given nodes would form a loop".into(),
            ));
        }
        Ok(edge_index)
    }
    /// Export the optic graph into the `dot` format to be used in combination with the [`graphviz`](https://graphviz.org/) software.
    pub fn to_dot(&self) -> String {
        let mut dot_string = "digraph {\n".to_owned();
        dot_string.push_str(&format!("  label=\"{}\"\n", self.description));
        dot_string.push_str("  fontname=\"Helvetica,Arial,sans-serif\"\n");
        dot_string.push_str("  node [fontname=\"Helvetica,Arial,sans-serif\"]\n");
        dot_string.push_str("  edge [fontname=\"Helvetica,Arial,sans-serif\"]\n");
        for node_idx in self.g.node_indices() {
            let node = self.g.node_weight(node_idx).unwrap();
            dot_string += &node.to_dot(&format!("i{}", node_idx.index()));
        }
        for edge in self.g.edge_indices() {
            let end_nodes = self.g.edge_endpoints(edge).unwrap();
            dot_string.push_str(&format!(
                "  i{} -> i{}\n",
                end_nodes.0.index(),
                end_nodes.1.index()
            ));
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
    #[test]
    fn add_node() {
        let mut scenery = OpticScenery::new();
        scenery.add_node(OpticNode::new("Test", NodeDummy));
        assert_eq!(scenery.g.node_count(), 1);
    }
    #[test]
    fn add_element() {
        let mut scenery = OpticScenery::new();
        scenery.add_element("Test", NodeDummy);
        assert_eq!(scenery.g.node_count(), 1);
    }
    #[test]
    fn connect_nodes_ok() {
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_element("Test", NodeDummy);
        let n2 = scenery.add_element("Test", NodeDummy);
        assert!(scenery.connect_nodes(n1, "rear", n2, "front").is_ok());
        assert_eq!(scenery.g.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_failure() {
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_element("Test", NodeDummy);
        let n2 = scenery.add_element("Test", NodeDummy);
        assert!(scenery
            .connect_nodes(n1, "rear", NodeIndex::new(5), "front")
            .is_err());
        assert!(scenery
            .connect_nodes(NodeIndex::new(5), "rear", n2, "front")
            .is_err());
    }
    #[test]
    fn connect_nodes_loop_error() {
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_element("Test", NodeDummy);
        let n2 = scenery.add_element("Test", NodeDummy);
        assert!(scenery.connect_nodes(n1, "rear", n2, "front").is_ok());
        assert!(scenery.connect_nodes(n2, "rear", n1, "front").is_err());
        assert_eq!(scenery.g.edge_count(), 1);
    }
    #[test]
    fn to_dot_empty() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.to_dot(), "digraph {\n  label=\"Test\"\n  fontname=\"Helvetica,Arial,sans-serif\"\n  node [fontname=\"Helvetica,Arial,sans-serif\"]\n  edge [fontname=\"Helvetica,Arial,sans-serif\"]\n}");
    }
    #[test]
    fn to_dot_with_node() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into());
        scenery.add_element("Test", NodeDummy);
        assert_eq!(
            scenery.to_dot(),
            "digraph {\n  label=\"SceneryTest\"\n  fontname=\"Helvetica,Arial,sans-serif\"\n  node [fontname=\"Helvetica,Arial,sans-serif\"]\n  edge [fontname=\"Helvetica,Arial,sans-serif\"]\n  i0 [label=\"Test\"]\n}"
        );
    }
    #[test]
    fn to_dot_with_edge() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into());
        let n1 = scenery.add_element("Test1", NodeDummy);
        let n2 = scenery.add_element("Test2", NodeDummy);
        if let Ok(_) = scenery.connect_nodes(n1, "rear", n2, "front") {
            assert_eq!(
                scenery.to_dot(),
                "digraph {\n  label=\"SceneryTest\"\n  fontname=\"Helvetica,Arial,sans-serif\"\n  node [fontname=\"Helvetica,Arial,sans-serif\"]\n  edge [fontname=\"Helvetica,Arial,sans-serif\"]\n  i0 [label=\"Test1\"]\n  i1 [label=\"Test2\"]\n  i0 -> i1\n}"
            );
        } else {
            assert!(false);
        }
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
