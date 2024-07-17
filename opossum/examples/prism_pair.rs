use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, RayPropagationVisualizer, SpotDiagram, Wedge},
    optical::Alignable,
    refractive_index::RefrIndexConst,
    OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("Prism Pair test".into())?;

    let src = scenery.add_node(collimated_line_ray_source(
        millimeter!(50.0),
        joule!(1.0),
        7,
    )?);
    let prism1 = Wedge::new(
        "Prism1",
        millimeter!(20.0),
        degree!(30.0),
        &RefrIndexConst::new(1.5068)?,
    )?;
    let p1 = scenery.add_node(prism1);

    let prism2 = Wedge::new(
        "Prism2",
        millimeter!(20.0),
        degree!(-30.0),
        &RefrIndexConst::new(1.5068)?,
    )?
    .with_tilt(degree!(0.0, 0.0, 0.0))?;
    let p2 = scenery.add_node(prism2);

    let det = scenery.add_node(RayPropagationVisualizer::default());
    let sd = scenery.add_node(SpotDiagram::default());
    //let wf = scenery.add_node(WaveFront::new("Wavefront"));

    scenery.connect_nodes(src, "out1", p1, "front", millimeter!(10.0))?;
    scenery.connect_nodes(p1, "rear", p2, "front", millimeter!(100.0))?;

    scenery.connect_nodes(p2, "rear", sd, "in1", millimeter!(50.0))?;
    scenery.connect_nodes(sd, "out1", det, "in1", millimeter!(0.1))?;
    //scenery.connect_nodes(l2, "rear", wf, "in1", millimeter!(50.0))?;

    scenery.save_to_file(Path::new("./opossum/playground/prism_pair.opm"))?;
    Ok(())
}
