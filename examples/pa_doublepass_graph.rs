use opossum::nodes::{NodeDummy, NodeReference};
use opossum::optic_node::OpticNode;
use opossum::optic_scenery::OpticScenery;

use std::fs::File;
use std::io::Write;

fn main() {
    let mut scenery = OpticScenery::new();
    scenery.set_description("PreAmp Doublepass section".into());
    let n1 = scenery.add_node(OpticNode::new("TFP", Box::new(NodeDummy)));
    let n2 = scenery.add_node(OpticNode::new("19mm amp", Box::new(NodeDummy)));
    let n3 = scenery.add_node(OpticNode::new("Faraday", Box::new(NodeDummy)));
    let n4 = scenery.add_node(OpticNode::new("0° mirror", Box::new(NodeDummy)));


    //let ref_node= NodeReference::new(&Box::new(NodeDummy));

    let mut node= OpticNode::new("Faraday", Box::new(NodeDummy));
    node.set_inverted(true);
    let n3i=scenery.add_node(node);

    let mut node= OpticNode::new("19mm amp", Box::new(NodeDummy));
    node.set_inverted(true);
    let n2i=scenery.add_node(node);

    let mut node= OpticNode::new("TFP", Box::new(NodeDummy));
    node.set_inverted(true);    
    let n1i=scenery.add_node(node);

    scenery.connect_nodes(n1, n2).unwrap();
    scenery.connect_nodes(n2, n3).unwrap();
    scenery.connect_nodes(n3, n4).unwrap();
    scenery.connect_nodes(n4, n3i).unwrap();
    scenery.connect_nodes(n3i, n2i).unwrap();
    scenery.connect_nodes(n2i, n1i).unwrap();

    let path = "graph.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
}
