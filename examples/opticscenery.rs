use opossum::error::OpossumError;
use opossum::nodes::Dummy;
use opossum::optic_scenery::OpticScenery;

use std::fs::File;
use std::io::Write;

fn main() -> Result<(), OpossumError> {
    println!("opticscenery example");
    let mut scenery = OpticScenery::new();
    scenery.set_description("OpticScenery demo".into());
    println!("default opticscenery: {:?}", scenery);
    println!("export to `dot` format: {}", scenery.to_dot()?);
    let node1 = scenery.add_element("my optic", Dummy);
    let node2 = scenery.add_element("my other optic", Dummy);
    scenery.connect_nodes(node1, "rear", node2, "front")?;
    let path = "graph.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()?).unwrap();

    Ok(())
}
