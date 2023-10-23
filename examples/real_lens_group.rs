use opossum::error::OpossumError;
use opossum::nodes::{Dummy, NodeGroup};
use opossum::OpticScenery;
use std::path::Path;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();

    let mut group1 = NodeGroup::new("real lens group");
    group1.expand_view(true);
    let g1_n1 = group1.add_node(Dummy::new("surface"))?;
    let g1_n2 = group1.add_node(Dummy::new("material propagation"))?;
    let g1_n3 = group1.add_node(Dummy::new("surface"))?;
    group1.connect_nodes(g1_n1, "rear", g1_n2, "front")?;
    group1.connect_nodes(g1_n2, "rear", g1_n3, "front")?;

    let _scene_g1 = scenery.add_node(group1);
    scenery.save_to_file(Path::new("playground/real_lens_group.opm"))?;
    Ok(())
}
