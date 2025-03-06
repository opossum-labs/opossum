use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        collimated_line_ray_source, BeamSplitter, Lens, NodeGroup, RayPropagationVisualizer,
        ThinMirror,
    },
    optic_node::Alignable,
    OpmDocument,
};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("group test");

    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        6,
    )?)?;
    let mut group1 = NodeGroup::new("group 1");
    group1.set_expand_view(true)?;
    let i_g1_l = group1.add_node(Lens::default())?;
    group1.map_input_port(&i_g1_l, "input_1", "input_1")?;
    let i_g1_bs = group1.add_node(BeamSplitter::default())?;
    group1.connect_nodes(&i_g1_l, "output_1", &i_g1_bs, "input_1", millimeter!(100.0))?;
    let i_g1_m = group1.add_node(ThinMirror::default().with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    group1.connect_nodes(
        &i_g1_bs,
        "out1_trans1_refl2",
        &i_g1_m,
        "input_1",
        millimeter!(50.0),
    )?;
    group1.map_output_port(&i_g1_bs, "out2_trans2_refl1", "output1")?;
    group1.map_output_port(&i_g1_m, "output_1", "output2")?;

    let scene_g1 = scenery.add_node(group1)?;

    scenery.connect_nodes(&i_src, "output_1", &scene_g1, "input_1", millimeter!(50.0))?;

    let i_prop1 = scenery.add_node(RayPropagationVisualizer::new("direct", None)?)?;
    let i_prop2 = scenery.add_node(RayPropagationVisualizer::new("mirrored", None)?)?;

    scenery.connect_nodes(
        &scene_g1,
        "output1",
        &i_prop1,
        "input_1",
        millimeter!(100.0),
    )?;
    scenery.connect_nodes(
        &scene_g1,
        "output2",
        &i_prop2,
        "input_1",
        millimeter!(150.0),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/group_test.opm"))
}
