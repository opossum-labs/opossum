use opossum::nodes::NodeDummy;
use opossum::optic_node::OpticNode;
use opossum::optic_scenery::OpticScenery;

use std::fs::File;
use std::io::Write;

fn main() {
    println!("opticscenery example");
    let mut scenery = OpticScenery::new();
    scenery.set_description("OpticScenery demo".into());
    println!("default opticscenery: {:?}", scenery);
    println!("export to `dot` format: {}", scenery.to_dot());
    let node1 = scenery.add_node(OpticNode::new("my optic", Box::new(NodeDummy)));
    let node2 = scenery.add_node(OpticNode::new("my other optic", Box::new(NodeDummy)));
    if let Ok(_) = scenery.connect_nodes(node1, node2) {
        let path = "graph.dot";
        let mut output = File::create(path).unwrap();
        write!(output, "{}", scenery.to_dot()).unwrap();
    }
}
