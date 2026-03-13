use opossum_core::{nodes::round_collimated_ray_builder, prelude::*};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let src = scenery.add_node(SourcePort::default())?;

    let wf = scenery.add_node(WaveFront::default().with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let sd = scenery.add_node(SpotDiagram::default().with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let det = scenery.add_node(RayPropagationVisualizer::default())?;
    scenery.connect_nodes(src, "output_1", wf, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(wf, "output_1", sd, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(sd, "output_1", det, "input_1", millimeter!(20.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        src,
        round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 5)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/tilted_wavefront_sensor.opm",
    ))
}
