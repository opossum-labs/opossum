use opossum::error::OpmResult;
use opossum::lightdata::{DataEnergy, LightData};
use opossum::nodes::{EnergyMeter, RealLens, Source};
use opossum::spectrum_helper::create_he_ne_spec;
use opossum::OpticScenery;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;
    let src = scenery.add_node(Source::new(
        "Source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ));
    let l1 = scenery.add_node(RealLens::default()); // Lens 1
    let l2 = scenery.add_node(RealLens::default()); // Lens 2
    let det = scenery.add_node(EnergyMeter::default());

    scenery.connect_nodes(src, "out1", l1, "in1")?;
    scenery.connect_nodes(l1, "out1", l2, "in1")?;
    scenery.connect_nodes(l2, "out1", det, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/lens_system.opm"))?;
    Ok(())
}
