use nalgebra::Point2;
use opossum::aperture::{Aperture, CircleConfig};
use opossum::error::OpmResult;
use opossum::lightdata::{DataEnergy, LightData};
use opossum::nodes::{Dummy, EnergyMeter, Source};
use opossum::optical::Optical;
use opossum::spectrum::create_he_ne_spec;
use opossum::OpticScenery;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("OpticScenery demo")?;
    let node1 = scenery.add_node(Source::new(
        "Source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ));
    let mut dummy = Dummy::new("optic");
    dummy.set_input_aperture(
        "front",
        Aperture::BinaryCircle(CircleConfig::new(1.5, Point2::new(1.0, 1.0))?),
    )?;
    let node2 = scenery.add_node(dummy);
    let node3 = scenery.add_node(EnergyMeter::default());
    scenery.connect_nodes(node1, "out1", node2, "front")?;
    scenery.connect_nodes(node2, "rear", node3, "in1")?;
    scenery.save_to_file(Path::new("playground/simple_for_talk.opm"))?;
    Ok(())
}
