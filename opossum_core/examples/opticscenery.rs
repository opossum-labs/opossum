use opossum_core::prelude::*;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("OpticScenery demo");
    let node1 = scenery.add_node(Dummy::new("dummy1"))?;
    let node2 = scenery.add_node(Dummy::new("dummy2"))?;
    scenery.connect_nodes(node1, "output_1", node2, "input_1", millimeter!(0.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum_core/playground/opticscenery.opm"))
}
