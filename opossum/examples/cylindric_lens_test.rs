use nalgebra::Vector3;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, CylindricLens, NodeGroup, RayPropagationVisualizer,
        SpotDiagram,
    },
    optic_node::Alignable,
    refractive_index::RefrIndexConst,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();

    let src = scenery.add_node(&round_collimated_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        10,
    )?)?;
    let lens = CylindricLens::new(
        "Lens 1",
        millimeter!(100.0),
        millimeter!(f64::INFINITY),
        millimeter!(5.0),
        &RefrIndexConst::new(1.5068)?,
    )?
    .with_tilt(degree!(0.0, 0.0, 45.0))?;
    let l1 = scenery.add_node(&lens)?;
    let det = scenery.add_node(&RayPropagationVisualizer::new(
        "Ray_positions",
        Some(Vector3::y()),
    )?)?;
    let det2 = scenery.add_node(&SpotDiagram::default())?;
    scenery.connect_nodes(&src, "output_1", &l1, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(&l1, "output_1", &det, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(&det, "output_1", &det2, "input_1", millimeter!(0.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/cylindric_lens_test.opm"))
}
