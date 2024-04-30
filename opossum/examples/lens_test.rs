use num::Zero;
use opossum::{
    error::OpmResult,
    joule, millimeter,
    nodes::{round_collimated_ray_source, Lens, RayPropagationVisualizer, WaveFront},
    refractive_index::RefrIndexConst,
    OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;

    let src = scenery.add_node(round_collimated_ray_source(
        millimeter!(5.0),
        joule!(1.0),
        10,
    )?);
    let l1 = scenery.add_node(Lens::new(
        "Lens 1",
        millimeter!(205.55),
        millimeter!(-205.55),
        millimeter!(2.79),
        &RefrIndexConst::new(1.5068).unwrap(),
    )?);
    let l2 = scenery.add_node(Lens::new(
        "Lens 2",
        millimeter!(205.55),
        millimeter!(-205.55),
        millimeter!(2.79),
        &RefrIndexConst::new(1.5068).unwrap(),
    )?);
    let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot"));
    let wf = scenery.add_node(WaveFront::new("Wavefront"));
    scenery.connect_nodes(src, "out1", l1, "front", millimeter!(30.0))?;
    scenery.connect_nodes(l1, "rear", l2, "front", millimeter!(404.44560))?;
    scenery.connect_nodes(l2, "rear", wf, "in1", millimeter!(50.0))?;
    scenery.connect_nodes(wf, "out1", det, "in1", Length::zero())?;

    scenery.save_to_file(Path::new("./opossum/playground/lens_system.opm"))?;
    Ok(())
}
