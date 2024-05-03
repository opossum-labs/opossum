use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, RayPropagationVisualizer, SpotDiagram, Wedge},
    optical::Optical,
    refractive_index::RefrIndexConst,
    utils::geom_transformation::Isometry,
    OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Prism Pair test".into())?;

    let src = scenery.add_node(collimated_line_ray_source(
        millimeter!(50.0),
        joule!(1.0),
        7,
    )?);
    let prism1 = Wedge::new(
        "Prism1",
        millimeter!(20.0),
        degree!(0.0),
        &RefrIndexConst::new(1.5068)?,
    )?;
    let p1 = scenery.add_node(prism1);
    let mut prism2 = Wedge::new(
        "Prism2",
        millimeter!(20.0),
        degree!(0.0),
        &RefrIndexConst::new(1.5068).unwrap(),
    )?;
    let prism2_align = Some(Isometry::new(
        millimeter!(0.0, 0.0, 0.0),
        degree!(0.0, 0.0, 0.0),
    )?);
    prism2.set_property("alignment", prism2_align.into())?;
    let p2 = scenery.add_node(prism2);

    let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot"));
    let sd = scenery.add_node(SpotDiagram::default());
    //let wf = scenery.add_node(WaveFront::new("Wavefront"));

    scenery.connect_nodes(src, "out1", p1, "front", millimeter!(10.0))?;
    scenery.connect_nodes(p1, "rear", p2, "front", millimeter!(100.0))?;

    scenery.connect_nodes(p2, "rear", det, "in1", millimeter!(20.0))?;
    scenery.connect_nodes(det, "out1", sd, "in1", millimeter!(0.0))?;
    //scenery.connect_nodes(l2, "rear", wf, "in1", millimeter!(50.0))?;

    scenery.save_to_file(Path::new("./opossum/playground/prism_pair.opm"))?;
    Ok(())
}
