use std::fs::File;
use std::io::Write;

use opossum::{
    analyzer::AnalyzerEnergy,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    nodes::{Detector, Dummy, NodeGroup, Source},
    optic_node::OpticNode,
    optic_scenery::OpticScenery,
    spectrum::create_he_ne_spectrum,
};

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("optic group demo");

    let i_s = scenery.add_element(
        "Source",
        Source::new(LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        })),
    );
    let i_d = scenery.add_element("Detector", Detector::default());

    let mut group = NodeGroup::new();
    let i_g_d1 = group.add_node(OpticNode::new("dummy1", Dummy));
    let i_g_d2 = group.add_node(OpticNode::new("dummy2", Dummy));
    group.connect_nodes(i_g_d1, "rear", i_g_d2, "front")?;
    group.map_input_port(i_g_d1, "front", "input")?;
    group.map_output_port(i_g_d2, "rear", "output")?;
    let i_g = scenery.add_element("test group", group);

    scenery.connect_nodes(i_s, "out1", i_g, "input")?;
    scenery.connect_nodes(i_g, "output", i_d, "in1")?;

    let path = "group_test.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();

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
