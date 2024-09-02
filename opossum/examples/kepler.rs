use opossum::{
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, ParaxialSurface, RayPropagationVisualizer},
    optical::Optical,
    OpmDocument, OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        3,
    )?);
    let mut lens1 = ParaxialSurface::new("100 mm lens", millimeter!(100.0))?;
    let circle = CircleConfig::new(millimeter!(25.), millimeter!(0., 0.))?;
    lens1.set_input_aperture("front", &Aperture::BinaryCircle(circle))?;
    let i_pl1 = scenery.add_node(lens1);
    let i_pl2 = scenery.add_node(ParaxialSurface::new("50 mm lens", millimeter!(50.0))?);
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::new("after telecope", None)?);
    scenery.connect_nodes(i_src, "out1", i_pl1, "front", millimeter!(50.0))?;
    scenery.connect_nodes(i_pl1, "rear", i_pl2, "front", millimeter!(150.0))?;
    scenery.connect_nodes(i_pl2, "rear", i_sd3, "in1", millimeter!(50.0))?;
    OpmDocument::new(scenery).save_to_file(Path::new("./opossum/playground/kepler.opm"))
}
