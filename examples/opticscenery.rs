use opossum::optic_scenery::OpticScenery;
use opossum::optic_node::OpticNode;
use std::fs::File;
use std::io::Write;

fn main() {
    println!("opticscenery example");
    let mut scenery = OpticScenery::new();
    scenery.set_description("OpticScenery demo".into());
    println!("default opticscenery: {:?}", scenery);
    println!("export to `dot` format: {}", scenery.to_dot());
    scenery.add_node(OpticNode::new("my optic".into()));
    let path = "graph.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
}
