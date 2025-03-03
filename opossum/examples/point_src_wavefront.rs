use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, meter,
    nodes::{point_ray_source, NodeGroup, WaveFront},
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let source = point_ray_source(degree!(90.0), joule!(1.))?;
    let i_s = scenery.add_node(&source)?;
    let i_wf1 = scenery.add_node(&WaveFront::new("wf_monitor 1"))?;

    scenery.connect_nodes(&i_s, "output_1", &i_wf1, "input_1", meter!(0.1))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/point_src_wavefront.opm"))
}
