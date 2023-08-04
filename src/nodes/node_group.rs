use std::collections::{HashMap,HashSet};
use std::cell::Ref;
use crate::error::OpossumError;
use crate::light::Light;
use crate::optic_node::Dottable;
use crate::{
    optic_node::{OpticNode, Optical},
    optic_ports::OpticPorts,
};
use petgraph::algo::*;
use petgraph::prelude::{DiGraph, EdgeIndex, NodeIndex};

type Result<T> = std::result::Result<T, OpossumError>;

#[derive(Default, Debug)]
/// A node that represents a group of other [`OpticNode`]s. These subnodes are arranged in its own subgraph. All unconnected input and output ports of this subgraph form the ports of this [`NodeGroup`].
pub struct NodeGroup {
    g: DiGraph<OpticNode, Light>,
    linked_ports: HashMap<String, Vec<port_link>>,
    expand_view: bool,
}

#[derive(Default, Debug)]
pub struct port_link{
    pub port_name: String,
    pub node_idx: NodeIndex,
}

impl NodeGroup {
    pub fn new() -> Self {
        Self{
            expand_view: false,
            ..Default::default()
        }
    }
    /// Add a given [`OpticNode`] to the (sub-)graph of this [`NodeGroup`].
    ///
    /// This command just adds an [`OpticNode`] but does not connect it to existing nodes in the (sub-)graph. The given node is
    /// consumed (owned) by the [`NodeGroup`].
    pub fn add_node(&mut self, node: OpticNode) -> NodeIndex {
        self.g.add_node(node)
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
        let edge_index = self
            .g
            .add_edge(src_node, target_node, Light::new(src_port, target_port));
        if is_cyclic_directed(&self.g) {
            self.g.remove_edge(edge_index);
            return Err(OpossumError::OpticScenery(
                "connecting the given nodes would form a loop".into(),
            ));
        }
        Ok(edge_index)
    }
    fn src_node_port_exists(&self, src_node: NodeIndex, src_port: &str) -> bool {
        self.g
            .edges_directed(src_node, petgraph::Direction::Outgoing)
            .any(|e| e.weight().src_port() == src_port)
    }
    fn target_node_port_exists(&self, target_node: NodeIndex, target_port: &str) -> bool {
        self.g
            .edges_directed(target_node, petgraph::Direction::Incoming)
            .any(|e| e.weight().target_port() == target_port)
    }

    pub fn insert_port_link(&mut self, key: String, port_to_link_to: port_link){
        if !self.linked_ports.contains_key(&key){
            self.linked_ports.insert(key, vec![port_to_link_to]);
        }
        else{
            self.linked_ports
                .get_mut(&key)
                .unwrap()
                .push(port_to_link_to);
        }
    }

    pub fn link_ports(&mut self, group_port: &str, node_dest_index: NodeIndex, port_dest_name: &str){
        let dest_node_port = port_link{node_idx: node_dest_index, port_name: port_dest_name.to_owned()};
        self.insert_port_link(group_port.to_owned(), dest_node_port);
    }

    pub fn shall_expand(&self) -> bool{
        self.expand_view
    }

    pub fn get_linked_port_str(&self, port_name: &str, group_node_index_str: usize, mut parent_identifier: String) -> Result<Vec<String>> {
        if self.shall_expand() & self.linked_ports.contains_key(port_name){
            let linked_port = self.linked_ports.get(port_name).unwrap();
            let port_str  = linked_port.iter()
                       .map(|s| format!("{}_i{}:{}", parent_identifier, s.node_idx.index(), s.port_name)).collect();

            Ok(port_str)
        }
        else{
            Ok(vec![format!("{}:{}", parent_identifier, port_name)])
        }
    }

    pub fn expand_view(&mut self, expand_view:bool){
        self.expand_view = expand_view;
    }

    fn cast_node_to_group<'a>(&self, ref_node:  &'a OpticNode) -> Result<&'a  NodeGroup>{
        let node_boxed = (&*ref_node).node();
        let downcasted_node = node_boxed.downcast_ref::<NodeGroup>().unwrap();
        match downcasted_node {
            i => Ok(i),
            _ => Err(OpossumError::OpticScenery(
                "can not cast OpticNode to specific type of NodeGroup!".into(),
            )),
        }
    }
    fn check_if_group(&self, node_ref:  &OpticNode) -> bool{
        if node_ref.node_type() == "group"{
            true
        }
        else{
            false
        }
    }

    fn define_node_edge_str(&self, end_node: NodeIndex, light_port: &str, mut parent_identifier: String) -> Result<Vec<String>>{
        let mut edge_str = Vec::<String>::new();
        let node = self.g.node_weight(end_node).unwrap();

        parent_identifier = if parent_identifier == "" {format!("i{}", end_node.index())} else {format!("{}_i{}", &parent_identifier, end_node.index())};

        if self.check_if_group(&node){
            let group_node = self.cast_node_to_group(&node)?;
            edge_str = group_node.get_linked_port_str(light_port, end_node.index(), parent_identifier)?;            
        }
        else{
            edge_str.push(format!("{}:{}", parent_identifier, light_port));
        }
        Ok(edge_str)
    }

    fn to_dot_expanded_view(&self,node_index: &str, name: &str, inverted: bool, _ports: &OpticPorts, mut parent_identifier: String) -> Result<String>{
        let inv_string = if inverted { "(inv)" } else { "" };
        parent_identifier = if parent_identifier == "" {format!("i{}", node_index)} else {format!("{}_i{}", &parent_identifier, node_index)};
        let mut dot_string = format!(
            "  subgraph {} {{\n\tlabel=\"{}{}\"\n\tfontsize=15\n\tcluster=true\n\t",
            parent_identifier, name, inv_string
        );
        let mut src_edge_str = Vec::<String>::new();
        let mut target_edge_str = Vec::<String>::new();

        for node_idx in self.g.node_indices() {
            let node = self.g.node_weight(node_idx).unwrap();
            dot_string += &node.to_dot(&format!("{}", node_idx.index()), parent_identifier.clone())?;
        }
        for edge in self.g.edge_indices() {
            let light: &Light = self.g.edge_weight(edge).unwrap();
            let end_nodes = self.g.edge_endpoints(edge).unwrap();

            src_edge_str = self.define_node_edge_str(end_nodes.0, light.src_port(), parent_identifier.clone())?;
            target_edge_str = self.define_node_edge_str(end_nodes.1, light.target_port(), parent_identifier.clone())?;

            for src in src_edge_str.iter(){
                println!("{}", src);
                for target in target_edge_str.iter(){
                    println!("{}", target);
                    dot_string.push_str(&format!("  {} -> {} \n", src, target));
                };
            };
        }
        dot_string += "}";
        Ok(dot_string)
        // for node_idx in self.g.node_indices() {
        //     let node = self.g.node_weight(node_idx).unwrap();
        //     dot_string += &node.to_dot(&format!("{}_i{}", node_index, node_idx.index()));
        // }
        // for edge in self.g.edge_indices() {
        //     let end_nodes = self.g.edge_endpoints(edge).unwrap();
        //     let light = self.g.edge_weight(edge).unwrap();
        //     dot_string.push_str(&format!(
        //         "      i{}_i{}:{} -> i{}_i{}:{}\n",
        //         node_index,
        //         end_nodes.0.index(),
        //         light.src_port(),
        //         node_index,
        //         end_nodes.1.index(),
        //         light.target_port()
        //     ));
        // }
        // dot_string += "}";
        // dot_string
    }

    fn to_dot_contracted_view(&self,node_index: &str, name: &str, inverted: bool, _ports: &OpticPorts, mut parent_identifier: String) -> Result<String>{
        let inv_string = if inverted { " (inv)" } else { "" };
        let node_name = format!("{}{}", name, inv_string);        
        parent_identifier = if parent_identifier == "" {format!("i{}", node_index)} else {format!("{}_i{}", &parent_identifier, node_index)};
        let mut dot_str = format!("\t{} [\n\t\tshape=plaintext\n", parent_identifier);
        let mut indent_level = 2;
        dot_str.push_str(&self.add_html_like_labels(&node_name, &mut indent_level, _ports, inverted));
        Ok(dot_str)
    }
}

impl Optical for NodeGroup {
    fn node_type(&self) -> &str {
        "group"
    }
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("in1").unwrap();
        ports.add_output("out1").unwrap();
        ports
    }
}

impl Dottable for NodeGroup {
    fn to_dot(&self, node_index: &str, name: &str, inverted: bool, _ports: &OpticPorts, parent_identifier: String) -> Result<String> {

        if self.expand_view {
            self.to_dot_expanded_view(node_index, name, inverted, _ports, parent_identifier)
        }
        else{
            self.to_dot_contracted_view(node_index, name, inverted, _ports, parent_identifier)
        }
    }

    fn node_color(&self) -> &str {
        "yellow"
    }
}
