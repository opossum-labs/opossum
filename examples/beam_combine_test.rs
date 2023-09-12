use std::fs::File;
use std::io::Write;

use opossum::{
    analyzer::AnalyzerEnergy,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, Detector, FilterType, IdealFilter, Source},
    spectrum::{create_he_ne_spectrum, create_nd_glass_spectrum, Spectrum},
    OpticScenery,
};

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("beam combiner demo");

    let i_s1 = scenery.add_element(
        "Source 1",
        Source::new(LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        })),
    );
    let i_s2 = scenery.add_element(
        "Source 2",
        Source::new(LightData::Energy(DataEnergy {
            spectrum: create_nd_glass_spectrum(1.0),
        })),
    );
    let i_bs = scenery.add_element("Beam splitter", BeamSplitter::new(0.5).unwrap());
    let filter_spectrum = Spectrum::from_csv("NE03B.csv")?;
    let i_f = scenery.add_element(
        "Filter",
        IdealFilter::new(FilterType::Spectrum(filter_spectrum))?,
    );
    let i_d1 = scenery.add_element("Detector 1", Detector::default());

    scenery.connect_nodes(i_s1, "out1", i_bs, "input1")?;
    scenery.connect_nodes(i_s2, "out1", i_bs, "input2")?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_f, "front")?;
    scenery.connect_nodes(i_f, "rear", i_d1, "in1")?;

    let path = "beam_combiner.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot("")?).unwrap();

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
