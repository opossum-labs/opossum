use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, NodeGroup, RayPropagationVisualizer, SpotDiagram, Wedge},
    optic_node::OpticNode,
    refractive_index::RefrIndexConst,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Prism Pair test");
    let src = scenery.add_node(collimated_line_ray_source(
        millimeter!(50.0),
        joule!(1.0),
        7,
    )?)?;
    let prism1 = Wedge::new(
        "Prism1",
        millimeter!(20.0),
        degree!(30.0),
        &RefrIndexConst::new(1.5068)?,
    )?;
    let p1 = scenery.add_node(prism1)?;

    let mut prism2 = Wedge::new(
        "Prism2",
        millimeter!(20.0),
        degree!(-30.0),
        &RefrIndexConst::new(1.5068)?,
    )?;
    let iso = Isometry::new(millimeter!(0.0, 20.0, 110.0), degree!(30.0, 0.0, 0.0))?;
    prism2.set_isometry(iso)?;
    let p2 = scenery.add_node(prism2)?;

    let det = scenery.add_node(RayPropagationVisualizer::default())?;
    let sd = scenery.add_node(SpotDiagram::default())?;

    scenery.connect_nodes(src, "output_1", p1, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(p1, "output_1", p2, "input_1", millimeter!(100.0))?;

    scenery.connect_nodes(p2, "output_1", sd, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(sd, "output_1", det, "input_1", millimeter!(0.1))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/prism_pair.opm"))
}
