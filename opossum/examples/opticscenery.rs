use num::Zero;
use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    nodes::{Dummy, NodeGroup},
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("OpticScenery demo");
    let node1 = scenery.add_node(Dummy::new("dummy1"))?;
    let node2 = scenery.add_node(Dummy::new("dummy2"))?;
    scenery.connect_nodes(node1, "output_1", node2, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/opticscenery.opm"))
}
