use num::Zero;
use opossum::{
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, Lens, Propagation, RayPropagationVisualizer, SpotDiagram,
        WaveFront,
    },
    optical::Optical,
    refractive_index::RefrIndexConst,
    OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    let src = scenery.add_node(round_collimated_ray_source(
        millimeter!(5.0),
        joule!(1.0),
        5,
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
    let mut lens = Lens::new(
        "l1",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?;
    let circle = CircleConfig::new(millimeter!(3.0), millimeter!(0., 0.))?;
    lens.set_output_aperture("rear", &Aperture::BinaryCircle(circle))?;
    let l2 = scenery.add_node(lens);
    let s3 = scenery.add_node(Propagation::new("s3", millimeter!(30.0))?);
    let det = scenery.add_node(RayPropagationVisualizer::default());
    let wf = scenery.add_node(WaveFront::default());
    let sd = scenery.add_node(SpotDiagram::default());
    scenery.connect_nodes(src, "out1", s1, "front", Length::zero())?;
    scenery.connect_nodes(s1, "rear", l1, "front", Length::zero())?;
    scenery.connect_nodes(l1, "rear", s2, "front", Length::zero())?;
    scenery.connect_nodes(s2, "rear", l2, "front", Length::zero())?;
    scenery.connect_nodes(l2, "rear", s3, "front", Length::zero())?;
    scenery.connect_nodes(s3, "rear", wf, "in1", Length::zero())?;
    scenery.connect_nodes(wf, "out1", det, "in1", Length::zero())?;
    scenery.connect_nodes(det, "out1", sd, "in1", Length::zero())?;

    scenery.save_to_file(Path::new("./opossum/playground/surface_wavefront.opm"))?;
    Ok(())
}
