use opossum::analyzer::AnalyzerEnergy;
use opossum::error::OpossumError;
use opossum::nodes::{Dummy, NodeReference};
use opossum::optic_scenery::OpticScenery;
use std::fs::File;
use std::io::Write;

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("PreAmp Doublepass section".into());
    //let n0 = scenery.add_element("LightSource", Source::default());
    let n1 = scenery.add_element("TFP", Dummy);
    let n2 = scenery.add_element("19mm amp", Dummy);
    //let n3 = scenery.add_element("Faraday", Dummy);
    let n4 = scenery.add_element("0° mirror", Dummy);

    let mut node = NodeReference::new(scenery.node(n1).unwrap());
    node.set_inverted(true);
    let n1r = scenery.add_node(node);

    // let mut node= NodeReference::new(scenery.node(n3).unwrap());
    // node.set_inverted(true);
    // let n3r = scenery.add_node(node);

    let mut node = NodeReference::new(scenery.node(n2)?);
    node.set_inverted(true);
    let n2r = scenery.add_node(node);

    // scenery.connect_nodes(n0, "out1", n1, "front").unwrap();
    scenery.connect_nodes(n1, "rear", n2, "front")?;
    scenery.connect_nodes(n2, "rear", n4, "front")?;
    //  scenery.connect_nodes(n3, "rear", n4, "front").unwrap();
    scenery.connect_nodes(n4, "rear", n2r, "rear")?;
    // scenery.connect_nodes(n3r, "front", n2r, "rear").unwrap();
    scenery.connect_nodes(n2r, "front", n1r, "rear")?;

    // let mut group = NodeGroup::new();
    // let g_n1 = group.add_node(OpticNode::new("Beamsplitter", Dummy));
    // let g_n2 = group.add_node(OpticNode::new("Lens", Dummy));
    // let g_n3 = group.add_node(OpticNode::new("Lens2", Dummy));
    // let g_n4  = group.add_node(OpticNode::new("Det", Detector::default()));

    // group.connect_nodes(g_n1, "rear", g_n2, "front").unwrap();
    // group.connect_nodes(g_n2, "rear", g_n3, "front").unwrap();
    // group.connect_nodes(g_n3, "rear", g_n4, "in1").unwrap();
    // scenery.add_node(OpticNode::new("CamBox", group));
    let path = "graph.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
    // write!(output, "{}", scenery.to_dot()).unwrap();

    let mut analyzer = AnalyzerEnergy::new(&scenery);
    print!("Analyze...");
    analyzer.analyze()?;
    println!("Sucessful");

    Ok(())
}
