use std::path::Path;

use nalgebra::Point2;
use num::Zero;
use opossum::{
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    nodes::{collimated_line_ray_source, ParaxialSurface, Propagation, RayPropagationVisualizer},
    optical::Optical,
    OpticScenery,
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::millimeter,
};

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        Length::new::<millimeter>(20.0),
        Energy::new::<joule>(1.0),
        10,
    )?);
    let i_sd1 = scenery.add_node(Propagation::new("50mm", Length::new::<millimeter>(50.0))?);
    let mut lens1 = ParaxialSurface::new("100 mm lens", Length::new::<millimeter>(100.0))?;
    let circle = CircleConfig::new(Length::new::<millimeter>(9.9), Point2::new(Length::zero(), Length::zero()))?;
    lens1.set_input_aperture("front", &Aperture::BinaryCircle(circle))?;
    let i_pl1 = scenery.add_node(lens1);
    let i_pr1 = scenery.add_node(Propagation::new(
        "100 mm",
        Length::new::<millimeter>(100.0),
    )?);
    let i_pr2 = scenery.add_node(Propagation::new("50 mm", Length::new::<millimeter>(50.0))?);
    let mut lens2 = ParaxialSurface::new("50 mm lens", Length::new::<millimeter>(50.0))?;
    let circle = CircleConfig::new(Length::new::<millimeter>(3.5), Point2::new(Length::zero(), Length::zero()))?;
    lens2.set_input_aperture("front", &Aperture::BinaryCircle(circle))?;
    let i_pl2 = scenery.add_node(lens2);
    let i_pr3 = scenery.add_node(Propagation::new("50 mm", Length::new::<millimeter>(50.0))?);
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::new("after telecope"));
    scenery.connect_nodes(i_src, "out1", i_sd1, "front")?;
    scenery.connect_nodes(i_sd1, "rear", i_pl1, "front")?;
    scenery.connect_nodes(i_pl1, "rear", i_pr1, "front")?;
    scenery.connect_nodes(i_pr1, "rear", i_pr2, "front")?;
    scenery.connect_nodes(i_pr2, "rear", i_pl2, "front")?;
    scenery.connect_nodes(i_pl2, "rear", i_pr3, "front")?;
    scenery.connect_nodes(i_pr3, "rear", i_sd3, "in1")?;
    scenery.save_to_file(Path::new("./opossum/playground/kepler.opm"))?;
    Ok(())
}
