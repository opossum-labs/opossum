use opossum_core::{nodes::point_ray_builder, prelude::*};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::new("point ray source"))?;
    let i_wf1 = scenery.add_node(WaveFront::default())?;

    scenery.connect_nodes(i_src, "output_1", i_wf1, "input_1", meter!(0.1))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(i_src, point_ray_builder(degree!(90.0), joule!(1.))?);
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/point_src_wavefront.opm",
    ))
}
