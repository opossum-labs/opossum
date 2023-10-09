use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

use crate::analyzer::AnalyzerType;
use crate::error::{OpmResult, OpossumError};
use crate::light::Light;
use crate::lightdata::LightData;
use crate::nodes::NodeGroup;
use crate::optical::{LightResult, OpticGraph, OpticRef, Optical};
use crate::properties::{Properties, Property, Proptype};
use petgraph::algo::*;
use petgraph::prelude::NodeIndex;
use petgraph::visit::EdgeRef;
use serde_derive::{Deserialize, Serialize};

/// Overall optical model and additional metatdata.
///
/// All optical elements ([`Optical`]s) have to be added to this structure in order
/// to be considered for an analysis.
///
/// # Example
///
/// ```rust
/// use opossum::OpticScenery;
/// use opossum::nodes::Dummy;
/// use opossum::error::OpossumError;
///
/// fn main() -> Result<(), OpossumError> {
///   let mut scenery = OpticScenery::new();
///   scenery.set_description("OpticScenery demo");
///   let node1 = scenery.add_node(Dummy::new("dummy1"));
///   let node2 = scenery.add_node(Dummy::new("dummy2"));
///   scenery.connect_nodes(node1, "rear", node2, "front")
/// }
///
/// ```
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpticScenery {
    #[serde(rename = "graph")]
    g: OpticGraph,
    #[serde(rename = "properties")]
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props.set("description", "".into());
    props
}

impl Default for OpticScenery {
    fn default() -> Self {
        Self {
            g: Default::default(),
            props: create_default_props(),
        }
    }
}
impl OpticScenery {
    /// Creates a new (empty) [`OpticScenery`].
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a given [`Optical`] (Source, Detector, Lens, etc.) to the graph of this [`OpticScenery`].
    ///
    /// This command just adds an [`Optical`] to the graph. It does not connect
    /// it to existing nodes in the graph. The given optical element is consumed (owned) by the [`OpticScenery`].
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> NodeIndex {
        self.g.0.add_node(OpticRef(Rc::new(RefCell::new(node))))
    }
    /// Connect (already existing) nodes denoted by the respective `NodeIndex`.
    ///
    /// Both node indices must exist. Otherwise an [`OpossumError::OpticScenery`] is returned. In addition, connections are
    /// rejected and an [`OpossumError::OpticScenery`] is returned, if the graph would form a cycle (loop in the graph).
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
    ) -> OpmResult<()> {
        let source = self
            .g
            .0
            .node_weight(src_node)
            .ok_or(OpossumError::OpticScenery(
                "source node with given index does not exist".into(),
            ))?;
        if !source
            .0
            .borrow()
            .ports()
            .outputs()
            .contains(&src_port.into())
        {
            return Err(OpossumError::OpticScenery(format!(
                "source node {} does not have a port {}",
                source.0.borrow().name(),
                src_port
            )));
        }
        let target = self
            .g
            .0
            .node_weight(target_node)
            .ok_or(OpossumError::OpticScenery(
                "target node with given index does not exist".into(),
            ))?;
        if !target
            .0
            .borrow()
            .ports()
            .inputs()
            .contains(&target_port.into())
        {
            return Err(OpossumError::OpticScenery(format!(
                "target node {} does not have a port {}",
                target.0.borrow().name(),
                target_port
            )));
        }

        if self.src_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "src node with given port {} is already connected",
                src_port
            )));
        }
        if self.target_node_port_exists(src_node, src_port) {
            return Err(OpossumError::OpticScenery(format!(
                "target node with given port {} is already connected",
                target_port
            )));
        }
        let edge_index =
            self.g
                .0
                .add_edge(src_node, target_node, Light::new(src_port, target_port));
        if is_cyclic_directed(&self.g.0) {
            self.g.0.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(
                "connecting the given nodes would form a loop".into(),
            ));
        }
        Ok(())
    }
    fn src_node_port_exists(&self, src_node: NodeIndex, src_port: &str) -> bool {
        self.g
            .0
            .edges_directed(src_node, petgraph::Direction::Outgoing)
            .any(|e| e.weight().src_port() == src_port)
    }
    fn target_node_port_exists(&self, target_node: NodeIndex, target_port: &str) -> bool {
        self.g
            .0
            .edges_directed(target_node, petgraph::Direction::Incoming)
            .any(|e| e.weight().target_port() == target_port)
    }
    /// Return a reference to the [`Optical`] specified by its node index.
    ///
    /// This function is mainly useful for setting up a reference node.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node(&self, node: NodeIndex) -> OpmResult<Rc<RefCell<dyn Optical>>> {
        if let Some(node) = self.g.0.node_weight(node) {
            Ok(node.0.clone())
        } else {
            Err(OpossumError::OpticScenery(
                "node index does not exist".into(),
            ))
        }
    }
    /// Export the optic graph, including ports, into the `dot` format to be used in combination with the [`graphviz`](https://graphviz.org/) software.
    pub fn to_dot(&self, rankdir: &str) -> OpmResult<String> {
        //check direction
        let rankdir = if rankdir != "LR" { "TB" } else { "LR" };

        let mut dot_string = self.add_dot_header(rankdir);

        for node_idx in self.g.0.node_indices() {
            let node = self.g.0.node_weight(node_idx).unwrap();
            let node_name = node.0.borrow().name().to_owned();
            let inverted = node.0.borrow().inverted();
            let ports = node.0.borrow().ports();
            dot_string += &node.0.borrow().to_dot(
                &format!("{}", node_idx.index()),
                &node_name,
                inverted,
                &ports,
                "".to_owned(),
                rankdir,
            )?;
        }
        for edge in self.g.0.edge_indices() {
            let light: &Light = self.g.0.edge_weight(edge).unwrap();
            let end_nodes = self.g.0.edge_endpoints(edge).unwrap();

            let src_edge_str =
                self.create_node_edge_str(end_nodes.0, light.src_port(), "".to_owned())?;
            let target_edge_str =
                self.create_node_edge_str(end_nodes.1, light.target_port(), "".to_owned())?;

            dot_string.push_str(&format!("  {} -> {} \n", src_edge_str, target_edge_str));
        }
        dot_string.push_str("}\n");
        Ok(dot_string)
    }
    /// Returns the dot-file header of this [`OpticScenery`] graph.
    fn add_dot_header(&self, rankdir: &str) -> String {
        let mut dot_string = String::from("digraph {\n\tfontsize = 8\n");
        dot_string.push_str("\tcompound = true;\n");
        dot_string.push_str(&format!("\trankdir = \"{}\";\n", rankdir));
        dot_string.push_str(&format!("\tlabel=\"{}\"\n", self.description()));
        dot_string.push_str("\tfontname=\"Helvetica,Arial,sans-serif\"\n");
        dot_string.push_str("\tnode [fontname=\"Helvetica,Arial,sans-serif\" fontsize = 10]\n");
        dot_string.push_str("\tedge [fontname=\"Helvetica,Arial,sans-serif\"]\n\n");
        dot_string
    }
    fn create_node_edge_str(
        &self,
        end_node: NodeIndex,
        light_port: &str,
        mut parent_identifier: String,
    ) -> OpmResult<String> {
        let node = self.g.0.node_weight(end_node).unwrap().0.borrow();
        parent_identifier = if parent_identifier.is_empty() {
            format!("i{}", end_node.index())
        } else {
            format!("{}_i{}", &parent_identifier, end_node.index())
        };

        if node.node_type() == "group" {
            let group_node: &NodeGroup = node.as_group()?;
            Ok(group_node.get_mapped_port_str(light_port, parent_identifier)?)
        } else {
            Ok(format!("i{}:{}", end_node.index(), light_port))
        }
    }
    /// Analyze this [`OpticScenery`] based on a given [`AnalyzerType`].
    pub fn analyze(&mut self, analyzer_type: &AnalyzerType) -> OpmResult<()> {
        let sorted = toposort(&self.g.0, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))?;
        for idx in sorted {
            let node = self.g.0.node_weight(idx).unwrap();
            let incoming_edges: HashMap<String, Option<LightData>> = self.incoming_edges(idx);
            let outgoing_edges = node
                .0
                .borrow_mut()
                .analyze(incoming_edges, analyzer_type)
                .map_err(|e| {
                    format!("analysis of node {} failed: {}", node.0.borrow().name(), e)
                })?;
            for outgoing_edge in outgoing_edges {
                self.set_outgoing_edge_data(idx, outgoing_edge.0, outgoing_edge.1)
            }
        }
        Ok(())
    }
    /// Sets the description of this [`OpticScenery`].
    pub fn set_description(&mut self, description: &str) {
        self.props
            .set(
                "description",
                Property {
                    prop: Proptype::String(description.into()),
                },
            )
            .unwrap();
    }
    /// Returns a reference to the description of this [`OpticScenery`].
    pub fn description(&self) -> &str {
        let prop = self.props.get("description").unwrap();
        if let Proptype::String(dsc) = &prop.prop {
            dsc
        } else {
            ""
        }
    }
    fn incoming_edges(&self, idx: NodeIndex) -> LightResult {
        let edges = self.g.0.edges_directed(idx, petgraph::Direction::Incoming);
        edges
            .into_iter()
            .map(|e| {
                (
                    e.weight().target_port().to_owned(),
                    e.weight().data().cloned(),
                )
            })
            .collect::<HashMap<String, Option<LightData>>>()
    }
    fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: String, data: Option<LightData>) {
        let edges = self.g.0.edges_directed(idx, petgraph::Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_idx = edge_ref.id();
            let light = self.g.0.edge_weight_mut(edge_idx);
            if let Some(light) = light {
                light.set_data(data);
            }
        } // else outgoing edge not connected
    }
    pub fn report(&self, report_dir: &Path) -> serde_json::Value {
        let mut report = serde_json::Map::new();
        let detector_nodes = self
            .g
            .0
            .node_weights()
            .filter(|node| node.0.borrow().is_detector());
        //report.insert("detectors".into(), json!("Detectors"));
        let mut detectors: Vec<serde_json::Value> = Vec::new();
        for node in detector_nodes {
            detectors.push(node.0.borrow().report());
            node.0.borrow().export_data(report_dir);
        }
        let detector_json = serde_json::Value::Array(detectors);
        report.insert("detectors".into(), detector_json);
        serde_json::Value::Object(report)
    }
    pub fn save_to_file(&self, path: &Path) -> OpmResult<()> {
        let serialized = serde_json::to_string_pretty(&self).map_err(|e| {
            OpossumError::OpticScenery(format!("deserialization of OpticScenery failed: {}", e))
        })?;
        let mut output = File::create(path).map_err(|e| {
            OpossumError::OpticScenery(format!(
                "could not create file path: {}: {}",
                path.display(),
                e
            ))
        })?;
        write!(output, "{}", serialized).map_err(|e| {
            OpossumError::OpticScenery(format!(
                "writing to file path {} failed: {}",
                path.display(),
                e
            ))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::nodes::Metertype;

    use super::super::nodes::{BeamSplitter, Dummy, EnergyMeter, Source};
    use super::*;
    use std::{fs::File, io::Read};
    #[test]
    fn new() {
        let scenery = OpticScenery::new();
        assert_eq!(scenery.description(), "".to_owned());
        assert_eq!(scenery.g.0.edge_count(), 0);
        assert_eq!(scenery.g.0.node_count(), 0);
    }
    #[test]
    fn add_node() {
        let mut scenery = OpticScenery::new();
        scenery.add_node(Dummy::new("n1"));
        assert_eq!(scenery.g.0.node_count(), 1);
    }
    #[test]
    fn connect_nodes_ok() {
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_node(Dummy::new("Test"));
        let n2 = scenery.add_node(Dummy::new("Test"));
        assert!(scenery.connect_nodes(n1, "rear", n2, "front").is_ok());
        assert_eq!(scenery.g.0.edge_count(), 1);
    }
    #[test]
    fn connect_nodes_failure() {
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_node(Dummy::new("Test"));
        let n2 = scenery.add_node(Dummy::new("Test"));
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
        let n1 = scenery.add_node(Dummy::new("Test"));
        let n2 = scenery.add_node(Dummy::new("Test"));
        assert!(scenery.connect_nodes(n1, "rear", n2, "front").is_ok());
        assert!(scenery.connect_nodes(n2, "rear", n1, "front").is_err());
        assert_eq!(scenery.g.0.edge_count(), 1);
    }
    #[test]
    fn to_dot_empty() {
        let path = "files_for_testing/dot/to_dot_empty_TB.dot";
        let file_content_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_tb);

        let path = "files_for_testing/dot/to_dot_empty_LR.dot";
        let file_content_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_lr);

        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());

        let scenery_dot_str_tb = scenery.to_dot("TB").unwrap();
        let scenery_dot_str_lr = scenery.to_dot("LR").unwrap();

        assert_eq!(file_content_tb.clone(), scenery_dot_str_tb);
        assert_eq!(file_content_lr.clone(), scenery_dot_str_lr);
    }
    #[test]
    fn to_dot_with_node() {
        let path = "./files_for_testing/dot/to_dot_w_node_TB.dot";
        let file_content_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_tb);

        let path = "./files_for_testing/dot/to_dot_w_node_LR.dot";
        let file_content_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_lr);

        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into());
        scenery.add_node(Dummy::new("Test"));

        let scenery_dot_str_tb = scenery.to_dot("TB").unwrap();
        let scenery_dot_str_lr = scenery.to_dot("LR").unwrap();

        assert_eq!(file_content_tb.clone(), scenery_dot_str_tb);
        assert_eq!(file_content_lr.clone(), scenery_dot_str_lr);
    }
    #[test]
    fn to_dot_full() {
        let path = "files_for_testing/dot/to_dot_full_TB.dot";
        let file_content_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_tb);

        let path = "files_for_testing/dot/to_dot_full_LR.dot";
        let file_content_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_lr);

        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into());
        let i_s = scenery.add_node(Source::new("Source", LightData::Fourier));
        let mut bs = BeamSplitter::new(0.6).unwrap();
        bs.set_property("name", "Beam splitter".into()).unwrap();
        let i_bs = scenery.add_node(bs);
        let i_d1 = scenery.add_node(EnergyMeter::new(
            "Energy meter 1",
            Metertype::IdealEnergyMeter,
        ));
        let i_d2 = scenery.add_node(EnergyMeter::new(
            "Energy meter 2",
            Metertype::IdealEnergyMeter,
        ));

        scenery.connect_nodes(i_s, "out1", i_bs, "input1").unwrap();
        scenery
            .connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "in1")
            .unwrap();
        scenery
            .connect_nodes(i_bs, "out2_trans2_refl1", i_d2, "in1")
            .unwrap();

        let scenery_dot_str_tb = scenery.to_dot("TB").unwrap();
        let scenery_dot_str_lr = scenery.to_dot("LR").unwrap();

        assert_eq!(file_content_tb.clone(), scenery_dot_str_tb);
        assert_eq!(file_content_lr.clone(), scenery_dot_str_lr);
    }
    #[test]
    fn set_description() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.description(), "Test")
    }
    #[test]
    fn description() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into());
        assert_eq!(scenery.description(), "Test")
    }
}
