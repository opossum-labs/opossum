use nalgebra::Point2;
use opossum::aperture::{Aperture, CircleConfig};
use opossum::error::OpmResult;
use opossum::nodes::{
    round_collimated_ray_source, Lens, Propagation, RayPropagationVisualizer, SpotDiagram,
    WaveFront,
};
use opossum::optical::Optical;
use opossum::refractive_index::RefrIndexConst;
use opossum::OpticScenery;
use std::path::Path;
use uom::si::energy::joule;
use uom::si::f64::{Energy, Length};
use uom::si::length::millimeter;
use uom::num_traits::Zero;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    let src = scenery.add_node(round_collimated_ray_source(
        Length::new::<millimeter>(5.0),
        Energy::new::<joule>(1.0),
        5,
    )?);
    let s1 = scenery.add_node(Propagation::new("s1", Length::new::<millimeter>(30.0))?);
    let l1 = scenery.add_node(Lens::new(
        "l1",
        Length::new::<millimeter>(200.0),
        Length::new::<millimeter>(-200.0),
        Length::new::<millimeter>(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?);
    let s2 = scenery.add_node(Propagation::new(
        "s2",
        Length::new::<millimeter>(197.22992),
    )?);
    let mut lens = Lens::new(
        "l1",
        Length::new::<millimeter>(200.0),
        Length::new::<millimeter>(-200.0),
        Length::new::<millimeter>(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?;
    let circle = CircleConfig::new(Length::new::<millimeter>(3.0), Point2::new(Length::zero(), Length::zero()))?;
    lens.set_output_aperture("rear", &Aperture::BinaryCircle(circle))?;
    let l2 = scenery.add_node(lens);
    let s3 = scenery.add_node(Propagation::new("s3", Length::new::<millimeter>(30.0))?);
    let det = scenery.add_node(RayPropagationVisualizer::default());
    let wf = scenery.add_node(WaveFront::default());
    let sd = scenery.add_node(SpotDiagram::default());
    scenery.connect_nodes(src, "out1", s1, "front")?;
    scenery.connect_nodes(s1, "rear", l1, "front")?;
    scenery.connect_nodes(l1, "rear", s2, "front")?;
    scenery.connect_nodes(s2, "rear", l2, "front")?;
    scenery.connect_nodes(l2, "rear", s3, "front")?;
    scenery.connect_nodes(s3, "rear", wf, "in1")?;
    scenery.connect_nodes(wf, "out1", det, "in1")?;
    scenery.connect_nodes(det, "out1", sd, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/surface_wavefront.opm"))?;
    Ok(())
}
