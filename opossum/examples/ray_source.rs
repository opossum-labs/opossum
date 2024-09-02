use num::Zero;
use opossum::{
    aperture::{Aperture, CircleConfig}, error::OpmResult, joule, millimeter, nodes::{round_collimated_ray_source, Dummy, EnergyMeter, SpotDiagram}, optical::Optical, OpmDocument, OpticScenery
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("Raysource demo");
    let mut source = round_collimated_ray_source(millimeter!(1.0), joule!(1.0), 3)?;
    let aperture =
        Aperture::BinaryCircle(CircleConfig::new(millimeter!(1.0), millimeter![0.5, 0.5])?);
    source.set_output_aperture("out1", &aperture)?;
    let i_s = scenery.add_node(source);
    let dummy = Dummy::default();
    let i_dummy = scenery.add_node(dummy);
    let i_d = scenery.add_node(EnergyMeter::default());
    let i_sd = scenery.add_node(SpotDiagram::default());
    scenery.connect_nodes(i_s, "out1", i_dummy, "front", Length::zero())?;
    scenery.connect_nodes(i_dummy, "rear", i_d, "in1", Length::zero())?;
    scenery.connect_nodes(i_d, "out1", i_sd, "in1", Length::zero())?;

    OpmDocument::new(scenery).save_to_file(Path::new("./opossum/playground/ray_source.opm"))
}
