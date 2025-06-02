use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        NodeGroup, RayPropagationVisualizer, SpotDiagram, WaveFront, round_collimated_ray_source,
    },
    optic_node::Alignable,
};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let src =
        scenery.add_node(round_collimated_ray_source(millimeter!(5.0), joule!(1.0), 5).unwrap())?;
    let wf = scenery.add_node(WaveFront::default().with_tilt(degree!(10.0, 0.0, 0.0))?)?;
    let sd = scenery.add_node(SpotDiagram::default().with_tilt(degree!(30.0, 0.0, 0.0))?)?;
    let det = scenery.add_node(RayPropagationVisualizer::default())?;
    scenery.connect_nodes(src, "output_1", wf, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(wf, "output_1", sd, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(sd, "output_1", det, "input_1", millimeter!(20.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/tilted_wavefront_sensor.opm",
    ))
}
