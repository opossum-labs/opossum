use nalgebra::Vector3;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, EnergyMeter, NodeGroup, ParabolicMirror,
        RayPropagationVisualizer, SpotDiagram, WaveFront,
    },
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(&round_collimated_ray_source(
        millimeter!(240.0),
        joule!(1.0),
        8,
    )?)?;
    let i_m1 = scenery.add_node(&ParabolicMirror::new_with_off_axis_y(
        "parabola 1",
        millimeter!(400.0),
        false,
        degree!(45.0),
    )?)?;
    let i_m2 = scenery.add_node(&ParabolicMirror::new_with_off_axis_y(
        "parabola 2",
        millimeter!(50.0),
        true,
        degree!(-45.0),
    )?)?;
    let rpv =
        RayPropagationVisualizer::new("visualizer", Some(Vector3::new(10., 0., 0.).normalize()))?;
    let i_prop_vis = scenery.add_node(&rpv)?;
    let i_sd = scenery.add_node(&SpotDiagram::default())?;
    let i_wf = scenery.add_node(&WaveFront::default())?;
    let i_pm = scenery.add_node(&EnergyMeter::default())?;
    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(450.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_sd, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_sd, "output_1", i_wf, "input_1", millimeter!(0.1))?;
    scenery.connect_nodes(i_wf, "output_1", i_pm, "input_1", millimeter!(0.1))?;
    scenery.connect_nodes(i_pm, "output_1", i_prop_vis, "input_1", millimeter!(0.1))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/parabolic_mirror.opm"))
}
