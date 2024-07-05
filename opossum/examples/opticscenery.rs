use num::Zero;
use opossum::{error::OpmResult, nodes::Dummy, OpticScenery};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("OpticScenery demo")?;
    let node1 = scenery.add_node(Dummy::new("dummy1"));
    let node2 = scenery.add_node(Dummy::new("dummy2"));
    scenery.connect_nodes(node1, "rear", node2, "front", Length::zero())?;
    scenery.save_to_file(Path::new("./opossum/playground/opticscenery.opm"))?;
    Ok(())
}
