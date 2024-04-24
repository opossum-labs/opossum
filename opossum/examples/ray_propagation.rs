use num::Zero;
use opossum::{
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{round_collimated_ray_source, Lens, Propagation, RayPropagationVisualizer},
    optical::Optical,
    refractive_index::RefrIndexConst,
    OpticScenery,
};
use uom::si::f64::Length;
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    let src = scenery.add_node(round_collimated_ray_source(
        millimeter!(5.0),
        joule!(1.0),
        10,
    )?);
    let s1 = scenery.add_node(Propagation::new("s1", millimeter!(30.0))?);
    let l1 = scenery.add_node(Lens::new(
        "l1",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?);
    let s2 = scenery.add_node(Propagation::new("s2", millimeter!(197.22992))?);
    let mut lens2 = Lens::new(
        "l2",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?;
    let _ = lens2.set_input_aperture(
        "front",
        &Aperture::BinaryCircle(CircleConfig::new(millimeter!(3.), millimeter!(0., 0.)).unwrap()),
    );
    let l2 = scenery.add_node(lens2);
    let s3 = scenery.add_node(Propagation::new("s3", millimeter!(30.0))?);
    let det = scenery.add_node(RayPropagationVisualizer::default());
    scenery.connect_nodes(src, "out1", s1, "front", Length::zero())?;
    scenery.connect_nodes(s1, "rear", l1, "front", Length::zero())?;
    scenery.connect_nodes(l1, "rear", s2, "front", Length::zero())?;
    scenery.connect_nodes(s2, "rear", l2, "front", Length::zero())?;
    scenery.connect_nodes(l2, "rear", s3, "front", Length::zero())?;
    scenery.connect_nodes(s3, "rear", det, "in1", Length::zero())?;

    scenery.save_to_file(Path::new("./opossum/playground/ray_propagation.opm"))?;
    Ok(())
}
