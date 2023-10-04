use std::io::Write;
use std::{fs::File, path::Path};

use opossum::optical::Optical;
use opossum::{
    analyzer::AnalyzerEnergy,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, EnergyMeter, Source},
    spectrum::create_he_ne_spectrum,
    OpticScenery,
};

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("inverse beam splitter test");

    let i_s = scenery.add_node(Source::new(
        "Source",
        LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        }),
    ));
    let mut bs = BeamSplitter::new(0.6).unwrap();
    bs.set_property("inverted", true.into()).unwrap();
    let i_bs = scenery.add_node(bs);
    let i_d1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ));
    let i_d2 = scenery.add_node(EnergyMeter::new(
        "Energy meter 2",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ));

    scenery.connect_nodes(i_s, "out1", i_bs, "out1_trans1_refl2")?;
    scenery.connect_nodes(i_bs, "input1", i_d1, "in1")?;
    scenery.connect_nodes(i_bs, "input2", i_d2, "in1")?;

    let serialized = serde_json::to_string_pretty(&scenery).unwrap();
    let path = "inverse_beam_splitter_test.opm";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", serialized).unwrap();

    scenery.report(Path::new("./"));
    println!("");
    let mut analyzer = AnalyzerEnergy::new(&scenery);
    print!("Analyze...");
    analyzer.analyze()?;
    println!("Sucessful");
    println!("");
    scenery.report(Path::new("./"));

    Ok(())
}
