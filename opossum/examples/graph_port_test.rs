use opossum::error::OpmResult;
use opossum::nodes::{BeamSplitter, Dummy};

use opossum::OpticScenery;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Fancy Graph with Ports")?;

    let in1 = scenery.add_node(Dummy::new("test1"));
    let out1 = scenery.add_node(Dummy::new("test2"));
    let bs1 = scenery.add_node(BeamSplitter::default());
    let bs2 = scenery.add_node(BeamSplitter::default());
    let m1 = scenery.add_node(Dummy::new("test4"));
    let m2 = scenery.add_node(Dummy::new("test5"));

    scenery.connect_nodes(in1, "rear", bs1, "input1")?;
    scenery.connect_nodes(bs1, "out1_trans1_refl2", m1, "front")?;
    scenery.connect_nodes(bs1, "out2_trans2_refl1", m2, "front")?;

    scenery.connect_nodes(m1, "rear", bs2, "input1")?;
    scenery.connect_nodes(m2, "rear", bs2, "input2")?;
    scenery.connect_nodes(bs2, "out1_trans1_refl2", out1, "front")?;
    scenery.save_to_file(Path::new("./opossum/playground/graph_w_ports.opm"))?;
    Ok(())
}
