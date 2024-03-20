use std::path::Path;

use nalgebra::point;
use opossum::{
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    nodes::{round_collimated_ray_source, Dummy, EnergyMeter, SpotDiagram},
    optical::Optical,
    OpticScenery,
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::millimeter,
};

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Raysource demo")?;
    let mut source =
        round_collimated_ray_source(Length::new::<millimeter>(1.0), Energy::new::<joule>(1.0), 3)?;
    let aperture = Aperture::BinaryCircle(CircleConfig::new(Length::new::<millimeter>(1.0), point![Length::new::<millimeter>(0.5), Length::new::<millimeter>(0.5)])?);
    let mut ports = source.ports();
    ports.set_output_aperture("out1", &aperture)?;
    source.set_property("apertures", ports.into())?;
    let i_s = scenery.add_node(source);
    let dummy = Dummy::default();
    let i_dummy = scenery.add_node(dummy);
    let i_d = scenery.add_node(EnergyMeter::default());
    let i_sd = scenery.add_node(SpotDiagram::default());
    scenery.connect_nodes(i_s, "out1", i_dummy, "front")?;
    scenery.connect_nodes(i_dummy, "rear", i_d, "in1")?;
    scenery.connect_nodes(i_d, "out1", i_sd, "in1")?;
    scenery.save_to_file(Path::new("./opossum/playground/ray_source.opm"))?;
    Ok(())
}
