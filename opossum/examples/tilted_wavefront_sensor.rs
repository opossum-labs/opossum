use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, NodeGroup, RayPropagationVisualizer, SpotDiagram, WaveFront,
    },
    optical::Alignable,
    OpmDocument,
};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let src =
        scenery.add_node(round_collimated_ray_source(millimeter!(5.0), joule!(1.0), 5).unwrap())?;
    let wf = scenery.add_node(WaveFront::default().with_tilt(degree!(10.0, 0.0, 0.0))?)?;
    let sd = scenery.add_node(SpotDiagram::default().with_tilt(degree!(30.0, 0.0, 0.0))?)?;
    let det = scenery.add_node(RayPropagationVisualizer::default())?;
    scenery.connect_nodes(src, "out1", wf, "in1", millimeter!(20.0))?;
    scenery.connect_nodes(wf, "out1", sd, "in1", millimeter!(20.0))?;
    scenery.connect_nodes(sd, "out1", det, "in1", millimeter!(20.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/tilted_wavefront_sensor.opm",
    ))
}
