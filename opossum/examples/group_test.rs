use num::Zero;
use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        collimated_line_ray_source, BeamSplitter, Dummy, Lens, NodeGroup, RayPropagationVisualizer,
        ThinMirror,
    },
    optical::Alignable,
    OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();

    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        6,
    )?);
    let mut group1 = NodeGroup::new("group 1");
    group1.set_expand_view(true)?;
    let i_g1_l = group1.add_node(Lens::default())?;
    group1.map_input_port(i_g1_l, "front", "input")?;
    let i_g1_bs = group1.add_node(BeamSplitter::default())?;
    group1.connect_nodes(i_g1_l, "rear", i_g1_bs, "input1", millimeter!(100.0))?;
    let i_g1_m = group1.add_node(ThinMirror::default().with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    group1.connect_nodes(
        i_g1_bs,
        "out2_trans2_refl1",
        i_g1_m,
        "input",
        millimeter!(50.0),
    )?;

    let i_g1_m1 = group1.add_node(ThinMirror::default().with_tilt(degree!(-45.0, 0.0, 0.0))?)?;
    group1.connect_nodes(
        i_g1_bs,
        "out1_trans1_refl2",
        i_g1_m1,
        "input",
        millimeter!(50.0),
    )?;
    //group1.map_output_port(i_g1_bs, "out1_trans1_refl2", "output1")?;
    group1.map_output_port(i_g1_m1, "reflected", "output1")?;
    group1.map_output_port(i_g1_m, "reflected", "output2")?;

    let scene_g1 = scenery.add_node(group1);

    scenery.connect_nodes(i_src, "out1", scene_g1, "input", millimeter!(50.0))?;

    let i_prop1=scenery.add_node(RayPropagationVisualizer::new("direct"));
    let i_prop2 = scenery.add_node(RayPropagationVisualizer::new("mirrored"));

    scenery.connect_nodes(scene_g1, "output1", i_prop1, "in1", millimeter!(100.0))?;
    scenery.connect_nodes(scene_g1, "output2", i_prop2, "in1", millimeter!(100.0))?;

    // let d2 = scenery.add_node(Dummy::new("node2"));
    // scenery.connect_nodes(scene_g1, "output", d2, "front", Length::zero())?;
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
