use opossum::error::OpossumError;
use opossum::nodes::{BeamSplitter, Dummy, NodeGroup};
use opossum::OpticScenery;
use std::path::Path;
fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Node Group test section".into());

    let mut group1 = NodeGroup::default();
    group1.expand_view(true);
    let g1_n1 = group1.add_node(Dummy::new("node1"));
    let g1_n2 = group1.add_node(BeamSplitter::default());
    group1.map_output_port(g1_n2, "out1_trans1_refl2", "out1")?;
    group1.connect_nodes(g1_n1, "rear", g1_n2, "input1")?;

    let mut nested_group = NodeGroup::default();
    let nested_g_n1 = nested_group.add_node(Dummy::new("node1_1"));
    let nested_g_n2 = nested_group.add_node(Dummy::new("node1_2"));
    nested_group.expand_view(true);

    nested_group.connect_nodes(nested_g_n1, "rear", nested_g_n2, "front")?;
    nested_group.map_input_port(nested_g_n1, "front", "in1")?;
    nested_group.map_output_port(nested_g_n2, "rear", "out1")?;

    let nested_group_index = group1.add_node(nested_group);
    group1.connect_nodes(nested_group_index, "out1", g1_n1, "front")?;

    let mut group2: NodeGroup = NodeGroup::default();
    group2.expand_view(true);
    let g2_n1 = group2.add_node(Dummy::new("node2_1"));
    let g2_n2 = group2.add_node(Dummy::new("node2_2"));
    group2.map_input_port(g2_n1, "front", "in1")?;

    group2.connect_nodes(g2_n1, "rear", g2_n2, "front")?;

    let scene_g1 = scenery.add_node(group1);
    let scene_g2 = scenery.add_node(group2);

    // set_output_port
    scenery.connect_nodes(scene_g1, "out1", scene_g2, "in1")?;

    scenery.save_to_file(Path::new("group_test.opm"))?;
    Ok(())
}
