use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{NodeGroup, RayPropagationVisualizer, ThinMirror, collimated_line_ray_source},
    optic_node::Alignable,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let src = collimated_line_ray_source(millimeter!(20.0), joule!(1.0), 21)?
        .with_tilt(degree!(20.0, 0.0, 0.0))?;
    let i_src = scenery.add_node(src)?;
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let i_m2 = scenery.add_node(
        ThinMirror::new("mirror 2")
            .with_curvature(millimeter!(-100.0))?
            .with_tilt(degree!(45.0, 0.0, 0.0))?,
    )?;
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::default())?;

    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_sd3, "input_1", millimeter!(100.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/tilted_src.opm"))
}
