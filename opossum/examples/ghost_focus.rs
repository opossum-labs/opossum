use opossum::{
    analyzers::{AnalyzerType, GhostFocusConfig, RayTraceConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, NodeGroup, SpotDiagram},
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        3,
    )?)?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;
    scenery.connect_nodes(i_src, "out1", i_sd, "in1", millimeter!(50.0))?;
    let mut doc = OpmDocument::new(scenery);
    let config = GhostFocusConfig::default();
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    //doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/ghost_focus.opm"))
}
