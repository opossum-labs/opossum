use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{round_collimated_ray_source, CylindricLens, RayPropagationVisualizer, SpotDiagram},
    optical::Alignable,
    refractive_index::RefrIndexConst,
    OpmDocument, OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();

    let src = scenery.add_node(round_collimated_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        3,
    )?);
    let lens = CylindricLens::new(
        "Lens 1",
        millimeter!(100.0),
        millimeter!(f64::INFINITY),
        millimeter!(5.0),
        &RefrIndexConst::new(1.5068)?,
    )?
    .with_tilt(degree!(0.0, 0.0, 45.0))?;
    let l1 = scenery.add_node(lens);
    let det = scenery.add_node(RayPropagationVisualizer::default());
    let det2 = scenery.add_node(SpotDiagram::default());
    scenery.connect_nodes(src, "out1", l1, "front", millimeter!(50.0))?;
    scenery.connect_nodes(l1, "rear", det, "in1", millimeter!(100.0))?;
    scenery.connect_nodes(det, "out1", det2, "in1", millimeter!(0.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/cylindric_lens_test.opm"))
}
