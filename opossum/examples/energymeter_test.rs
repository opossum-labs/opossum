use std::path::Path;

use opossum::{
    error::OpmResult,
    lightdata::{DataEnergy, LightData},
    nodes::{EnergyMeter, Source},
    spectrum::create_he_ne_spec,
    OpticScenery,
};

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("energymeter demo")?;

    let i_s = scenery.add_node(Source::new(
        "Source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ));
    let i_d = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ));

    scenery.connect_nodes(i_s, "out1", i_d, "in1")?;
    scenery.save_to_file(Path::new("./opossum/playground/energymeter_test.opm"))?;
    Ok(())
}
