use std::fs::File;
use std::io::Write;

use opossum::{
    lightdata::{LightData, LightDataEnergy},
    nodes::{NodeDetector, NodeSource, NodeBeamSplitter},
    optic_scenery::OpticScenery, analyzer::AnalyzerEnergy,
};

fn main() {
    let mut scenery = OpticScenery::new();
    scenery.set_description("src - detector demo".into());

    let i_s = scenery.add_element(
        "Source",
        NodeSource::new(LightData::Energy(LightDataEnergy { energy: 1.0 })),
    );
    let i_bs=scenery.add_element("Beam splitter", NodeBeamSplitter::default());
    let i_d1 = scenery.add_element("Detector 1", NodeDetector::default());
    let i_d2 = scenery.add_element("Detector 2", NodeDetector::default());

    scenery.connect_nodes(i_s, "out1", i_bs, "input1").unwrap();

    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "in1").unwrap();
    scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_d2, "in1").unwrap();
    
    let path = "src_detector.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
    println!("{:?}", scenery.node_ref(i_s).unwrap());
    println!("{:?}", scenery.node_ref(i_d1).unwrap());

    let mut analyzer=AnalyzerEnergy::new(&scenery);
    print!("Analyze...");
    match analyzer.analyze() {
        Ok(_) => println!("Sucessful"),
        Err(e) => println!("Error: {}",e)
    }
    println!("{:?}", scenery.node_ref(i_d1).unwrap());
}
