use num::Zero;
use opossum::{
    error::OpmResult,
    joule, millimeter,
    nodes::{round_collimated_ray_source, Lens, Propagation, RayPropagationVisualizer, WaveFront},
    refractive_index::RefrIndexConst,
    OpticScenery,
};
use uom::si::f64::Length;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;

    let src = scenery.add_node(round_collimated_ray_source(
        millimeter!(5.0),
        joule!(1.0),
        10,
    )?);
    let s1 = scenery.add_node(Propagation::new("Dist 1", millimeter!(30.0))?);
    let l1 = scenery.add_node(Lens::new(
        "Lens 1",
        millimeter!(205.55),
        millimeter!(-205.55),
        millimeter!(2.79),
        &RefrIndexConst::new(1.5068).unwrap(),
    )?);
    let s2 = scenery.add_node(Propagation::new("Dist 2", millimeter!(404.44560))?);
    let l2 = scenery.add_node(Lens::new(
        "Lens 2",
        millimeter!(205.55),
        millimeter!(-205.55),
        millimeter!(2.79),
        &RefrIndexConst::new(1.5068).unwrap(),
    )?);
    let s3 = scenery.add_node(Propagation::new("Dist 3", millimeter!(50.0))?);
    let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot"));
    let wf = scenery.add_node(WaveFront::new("Wavefront"));
    scenery.connect_nodes(src, "out1", s1, "front", Length::zero())?;
    scenery.connect_nodes(s1, "rear", l1, "front", Length::zero())?;
    scenery.connect_nodes(l1, "rear", s2, "front", Length::zero())?;
    scenery.connect_nodes(s2, "rear", l2, "front", Length::zero())?;
    scenery.connect_nodes(l2, "rear", s3, "front",Length::zero())?;
    scenery.connect_nodes(s3, "rear", wf, "in1", Length::zero())?;
    scenery.connect_nodes(wf, "out1", det, "in1",Length::zero())?;

    scenery.save_to_file(Path::new("./opossum/playground/lens_system.opm"))?;
    Ok(())
}
