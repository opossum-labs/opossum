use opossum_core::{
    apertures::CircleShape,
    core_optics::{NodeAttrExt, node_attr::NodePositioning},
    geometry::body::CLEAR_APERTURE,
    prelude::*,
    refractive_index::RefrIndexConst,
    utils::geom_transformation::Isometry,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Prism Pair test");
    let src = scenery.add_node(SourcePort::new("collimated ray source"))?;
    // The beam meets the prisms up to 31 mm off their centers, so they are 64 mm in diameter and
    // still 1.5 mm thick at their thin edge. The outermost ray misses Prism2.
    let prism_size: ApertureShape = CircleShape::new(millimeter!(32.0))?.into();
    let mut prism1 = Wedge::new(
        "Prism1",
        millimeter!(20.0),
        degree!(30.0),
        RefrIndexConst::new(1.5068)?,
    )?;
    prism1.set_property(CLEAR_APERTURE, prism_size.clone().into())?;
    let p1 = scenery.add_node(prism1)?;

    let mut prism2 = Wedge::new(
        "Prism2",
        millimeter!(20.0),
        degree!(-30.0),
        RefrIndexConst::new(1.5068)?,
    )?;
    prism2.set_property(CLEAR_APERTURE, prism_size.into())?;
    prism2.set_positioning(NodePositioning::Absolute(Isometry::new(
        millimeter!(0.0, 20.0, 110.0),
        degree!(30.0, 0.0, 0.0),
    )?))?;
    let p2 = scenery.add_node(prism2)?;

    let mut rpv = RayPropagationVisualizer::default();
    rpv.set_property("ray transparency", 1.0.into())?;
    let det = scenery.add_node(rpv)?;
    let sd = scenery.add_node(SpotDiagram::default())?;

    scenery.connect_nodes(src, "output_1", p1, "input_1", millimeter!(10.0))?;
    scenery.connect_nodes(p1, "output_1", p2, "input_1", millimeter!(100.0))?;

    scenery.connect_nodes(p2, "output_1", sd, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(sd, "output_1", det, "input_1", millimeter!(0.1))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        src,
        collimated_line_ray_builder(millimeter!(50.0), joule!(1.0), 7)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/prism_pair.opm"))
}
