use opossum::{
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{round_collimated_ray_source, Lens, RayPropagationVisualizer},
    optical::Optical,
    refractive_index::RefrIndexConst,
    OpticScenery,
};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    let src = scenery.add_node(round_collimated_ray_source(
        millimeter!(5.0),
        joule!(1.0),
        10,
    )?);
    let l1 = scenery.add_node(Lens::new(
        "l1",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?);
    let mut lens2 = Lens::new(
        "l2",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?;
    lens2.set_input_aperture(
        "front",
        &Aperture::BinaryCircle(CircleConfig::new(millimeter!(3.), millimeter!(0., 0.))?),
    )?;
    let l2 = scenery.add_node(lens2);
    let det = scenery.add_node(RayPropagationVisualizer::default());
    scenery.connect_nodes(src, "out1", l1, "front", millimeter!(30.0))?;
    scenery.connect_nodes(l1, "rear", l2, "front", millimeter!(197.22992))?;
    scenery.connect_nodes(l2, "rear", det, "in1", millimeter!(30.0))?;

    scenery.save_to_file(Path::new("./opossum/playground/ray_propagation.opm"))?;
    Ok(())
}
