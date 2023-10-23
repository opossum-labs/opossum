use opossum::error::OpossumError;
use opossum::nodes::{Dummy, Source, EnergyMeter};
use opossum::OpticScenery;
use std::path::Path;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("OpticScenery demo");
    let node1 = scenery.add_node(Source::default());
    let node2 = scenery.add_node(Dummy::new("optic"));
    let node3 = scenery.add_node(EnergyMeter::default());
    scenery.connect_nodes(node1, "out1", node2, "front")?;
    scenery.connect_nodes(node2, "rear", node3, "in1")?;
    scenery.save_to_file(Path::new("playground/simple_for_talk.opm"))?;
    Ok(())
}
