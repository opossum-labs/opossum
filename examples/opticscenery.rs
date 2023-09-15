use opossum::error::OpossumError;
use opossum::nodes::Dummy;
use opossum::OpticScenery;

use std::fs::File;
use std::io::Write;

fn main() -> Result<(), OpossumError> {
    println!("opticscenery example");
    let mut scenery = OpticScenery::new();
    scenery.set_description("OpticScenery demo");
    let node1 = scenery.add_node(Dummy::new("dummy1"));
    let node2 = scenery.add_node(Dummy::new("dummy2"));
    scenery.connect_nodes(node1, "rear", node2, "front")?;
    println!("{}", serde_yaml::to_string(&scenery).unwrap());
    let path = "graph.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot("")?).unwrap();

    Ok(())
}
