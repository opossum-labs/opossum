use std::fs::File;
use std::io::Write;

use opossum::{
    lightdata::{LightData, LightDataEnergy},
    nodes::{NodeDetector, NodeSource},
    optic_scenery::OpticScenery, analyzer::AnalyzerEnergy,
};

fn main() {
    let mut scenery = OpticScenery::new();
    scenery.set_description("src - detector demo".into());

    let i_s = scenery.add_element(
        "Source",
        NodeSource::new(LightData::Energy(LightDataEnergy { energy: 1.0 })),
    );
    let i_d = scenery.add_element("Detector", NodeDetector::default());

    scenery.connect_nodes(i_s, "out1", i_d, "in1").unwrap();

    let path = "src_detector.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
    println!("{:?}", scenery.node_ref(i_s).unwrap());
    println!("{:?}", scenery.node_ref(i_d).unwrap());

    let analyzer=AnalyzerEnergy::new(&scenery);
    analyzer.analyze();
}
