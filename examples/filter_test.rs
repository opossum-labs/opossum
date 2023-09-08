use std::fs::File;
use std::io::Write;

use opossum::{
    analyzer::AnalyzerEnergy,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, Detector, EnergyMeter, FilterType, IdealFilter, Source, Spectrometer},
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
    let i_bs = scenery.add_element("Beam splitter", BeamSplitter::new(0.6).unwrap());
    let filter_spectrum = Spectrum::from_csv("NE03B.csv")?;
    let i_f = scenery.add_element(
        "Filter",
        IdealFilter::new(FilterType::Spectrum(filter_spectrum))?,
    );
    let i_d1 = scenery.add_element("Energy meter 1", Detector::default());
    let i_d2 = scenery.add_element("Spectrometer", Spectrometer::default());
    let i_d3 = scenery.add_element("Energy meter 2", EnergyMeter::default());

    scenery.connect_nodes(i_s, "out1", i_bs, "input1")?;

    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "in1")?;
    scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_f, "front")?;
    scenery.connect_nodes(i_f, "rear", i_d2, "in1")?;
    scenery.connect_nodes(i_d2, "out1", i_d3, "in1")?;

    let path = "src_detector.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()?).unwrap();

    scenery.report();
    println!("");
    let mut analyzer = AnalyzerEnergy::new(&scenery);
    print!("Analyze...");
    analyzer.analyze()?;
    println!("Sucessful");
    println!("");
    scenery.report();

    Ok(())
}
