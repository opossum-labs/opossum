use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    nodes::Dummy,
    OpmDocument, OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("OpticScenery demo");
    let node1 = scenery.add_node(Dummy::new("dummy1"));
    let node2 = scenery.add_node(Dummy::new("dummy2"));
    scenery.connect_nodes(node1, "rear", node2, "front", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/opticscenery.opm"))
}
