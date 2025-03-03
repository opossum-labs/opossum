use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, BeamSplitter, NodeGroup, RayPropagationVisualizer},
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src1 = scenery.add_node(&collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        21,
    )?)?;
    let i_src2 = scenery.add_node(&collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        21,
    )?)?;
    let i_bs = scenery.add_node(&BeamSplitter::default())?;
    let i_sd = scenery.add_node(&RayPropagationVisualizer::default())?;

    scenery.connect_nodes(&i_src1, "output_1", &i_bs, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(&i_src2, "output_1", &i_bs, "input_2", millimeter!(110.0))?;
    scenery.connect_nodes(
        &i_bs,
        "out1_trans1_refl2",
        &i_sd,
        "input_1",
        millimeter!(150.0),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/two_srcs.opm"))
}
