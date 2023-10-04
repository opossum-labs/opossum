use std::io::Write;
use std::{fs::File, path::Path};

use opossum::properties::{Property, Proptype};
use opossum::{
    analyzer::AnalyzerEnergy,
    error::OpossumError,
    lightdata::{DataEnergy, LightData},
    nodes::{Detector, Dummy, NodeGroup, Source},
    optical::Optical,
    spectrum::create_he_ne_spectrum,
    OpticScenery,
};

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Inverse Group test".into());

    let i_s = scenery.add_node(Source::new(
        "Source",
        LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spectrum(1.0),
        }),
    ));

    let mut group = NodeGroup::default();
    group.expand_view(true);
    let g_n1 = group.add_node(Dummy::new("node1"));
    let g_n2 = group.add_node(Dummy::new("node2"));

    group.connect_nodes(g_n1, "rear", g_n2, "front")?;
    group.map_input_port(g_n1, "front", "in1")?;
    group.map_output_port(g_n2, "rear", "out1")?;
    group.set_property("inverted", Property{prop: Proptype::Bool(true)}).unwrap();

    let i_g = scenery.add_node(group);
    let i_d = scenery.add_node(Detector::default());

    scenery.connect_nodes(i_s, "out1", i_g, "out1")?;
    scenery.connect_nodes(i_g, "in1", i_d, "in1")?;

    let path = "group_reverse.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot("")?).unwrap();

    let mut analyzer = AnalyzerEnergy::new(&scenery);
    analyzer.analyze()?;
    scenery.report(Path::new(""));

    Ok(())
}
