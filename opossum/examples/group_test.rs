use num::Zero;
use opossum::{
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, BeamSplitter, Dummy, Lens, NodeGroup},
    OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();

    let d0 = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        6,
    )?);
    let mut group1 = NodeGroup::new("group 1");
    group1.expand_view(true)?;
    let g1_n1 = group1.add_node(Lens::default())?;
    let g1_n2 = group1.add_node(BeamSplitter::default())?;
    group1.connect_nodes(g1_n1, "rear", g1_n2, "input1", Length::zero())?;
    group1.map_input_port(g1_n1, "front", "input")?;
    group1.map_output_port(g1_n2, "out1_trans1_refl2", "output")?;
    let scene_g1 = scenery.add_node(group1);

    scenery.connect_nodes(d0, "out1", scene_g1, "input", Length::zero())?;

    let d2 = scenery.add_node(Dummy::new("node2"));
    scenery.connect_nodes(scene_g1, "output", d2, "front", Length::zero())?;
    // let mut nested_group = NodeGroup::new("group 1_1");
    // let nested_g_n1 = nested_group.add_node(Dummy::new("node1_1"))?;
    // let nested_g_n2 = nested_group.add_node(Dummy::new("node1_2"))?;
    // nested_group.expand_view(true)?;

    // nested_group.connect_nodes(nested_g_n1, "rear", nested_g_n2, "front", Length::zero())?;
    // nested_group.map_input_port(nested_g_n1, "front", "in1")?;
    // nested_group.map_output_port(nested_g_n2, "rear", "out1")?;

    // let nested_group_index = group1.add_node(nested_group)?;
    // group1.connect_nodes(nested_group_index, "out1", g1_n1, "front", Length::zero())?;

    // let mut group2: NodeGroup = NodeGroup::new("group 2");
    // group2.expand_view(true)?;
    // let g2_n1 = group2.add_node(Dummy::new("node2_1"))?;
    // let g2_n2 = group2.add_node(Dummy::new("node2_2"))?;
    // group2.map_input_port(g2_n1, "front", "in1")?;

    // group2.connect_nodes(g2_n1, "rear", g2_n2, "front", Length::zero())?;

    // let scene_g2 = scenery.add_node(group2);

    // set_output_port
    // scenery.connect_nodes(scene_g1, "out1", scene_g2, "in1", Length::zero())?;
    scenery.save_to_file(Path::new("./opossum/playground/group_test.opm"))?;
    Ok(())
}
