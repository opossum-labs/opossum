use std::fs::File;
use std::io::Write;
use uom::si::{f64::Energy, energy::joule};

use opossum::{
    lightdata::{LightData, DataEnergy},
    nodes::{Detector, Source, BeamSplitter, IdealFilter},
    optic_scenery::OpticScenery, analyzer::AnalyzerEnergy,
};

fn main() {
    let mut scenery = OpticScenery::new();
    scenery.set_description("src - detector demo".into());

    let i_s = scenery.add_element(
        "Source",
        Source::new(LightData::Energy(DataEnergy { energy: Energy::new::<joule>(1.0) })),
    );
    let i_bs=scenery.add_element("Beam splitter", BeamSplitter::new(0.6));
    let i_f=scenery.add_element("Filter", IdealFilter::new(0.5).unwrap());
    let i_d1 = scenery.add_element("Detector 1", Detector::default());
    let i_d2 = scenery.add_element("Detector 2", Detector::default());

    scenery.connect_nodes(i_s, "out1", i_bs, "input1").unwrap();

    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "in1").unwrap();
    scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_f, "front").unwrap();
    scenery.connect_nodes(i_f, "rear", i_d2, "in1").unwrap();
    
    let path = "src_detector.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();

    scenery.report();
    println!("");
    let mut analyzer=AnalyzerEnergy::new(&scenery);
    print!("Analyze...");
    match analyzer.analyze() {
        Ok(_) => println!("Sucessful"),
        Err(e) => println!("Error: {}",e)
    }
    println!("");
    scenery.report();
}
