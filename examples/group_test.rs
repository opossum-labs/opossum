use std::fs::File;
use std::io::Write;

use opossum::{
    analyzer::AnalyzerEnergy,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    nodes::{BeamSplitter, Detector, Dummy, NodeGroup, Source},
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
    let i_d1 = scenery.add_element("Detector1", Detector::default());
    let i_d2 = scenery.add_element("Detector2", Detector::default());
    let mut group = NodeGroup::new();
    let i_g_d = group.add_node(OpticNode::new("dummy1", Dummy));
    let i_g_bs = group.add_node(OpticNode::new("bs", BeamSplitter::new(0.6)));
    group.connect_nodes(i_g_d, "rear", i_g_bs, "input1")?;
    group.map_input_port(i_g_d, "front", "input")?;
    group.map_output_port(i_g_bs, "out1_trans1_refl2", "output1")?;
    group.map_output_port(i_g_bs, "out2_trans2_refl1", "output2")?;
    let i_g = scenery.add_element("test group", group);

    scenery.connect_nodes(i_s, "out1", i_g, "input")?;
    scenery.connect_nodes(i_g, "output1", i_d1, "in1")?;
    scenery.connect_nodes(i_g, "output2", i_d2, "in1")?;
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
