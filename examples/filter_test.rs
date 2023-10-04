use std::io::Write;
use std::{fs::File, path::Path};

use opossum::{
    analyzer::AnalyzerEnergy,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, EnergyMeter, FilterType, IdealFilter, Source, Spectrometer},
    spectrum::{create_he_ne_spectrum, Spectrum},
    OpticScenery,
};

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("filter system demo");

    let i_s = scenery.add_node(Source::new(
        "Source",
        LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        }),
    ));
    let i_bs = scenery.add_node(BeamSplitter::new(0.6).unwrap());
    let filter_spectrum = Spectrum::from_csv("NE03B.csv")?;
    let i_f = scenery.add_node(IdealFilter::new(FilterType::Spectrum(filter_spectrum))?);
    let i_d1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ));
    let i_d2 = scenery.add_node(Spectrometer::default());
    let i_d3 = scenery.add_node(EnergyMeter::new(
        "Energy meter 2",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ));

    scenery.connect_nodes(i_s, "out1", i_bs, "input1")?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "in1")?;
    scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_f, "front")?;
    scenery.connect_nodes(i_f, "rear", i_d2, "in1")?;
    scenery.connect_nodes(i_d2, "out1", i_d3, "in1")?;

    let serialized = serde_json::to_string_pretty(&scenery).unwrap();
    let path = "filter_test.opm";
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
