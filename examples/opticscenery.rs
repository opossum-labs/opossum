use opossum::error::OpossumError;
use opossum::nodes::Dummy;
use opossum::OpticScenery;
use std::path::Path;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("OpticScenery demo");
    let node1 = scenery.add_node(Dummy::new("dummy1"));
    let node2 = scenery.add_node(Dummy::new("dummy2"));
    scenery.connect_nodes(node1, "rear", node2, "front")?;
    scenery.save_to_file(Path::new("playground/opticscenery.opm"))?;
    Ok(())
}
