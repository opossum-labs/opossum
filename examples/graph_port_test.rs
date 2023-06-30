use opossum::nodes::{NodeDummy, NodeBeamSplitter};
use opossum::optic_node::OpticNode;
use opossum::optic_scenery::OpticScenery;

use std::fs::File;
use std::io::Write;

fn main() {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Fancy Graph with Ports".into());

    let in1 = scenery.add_node(OpticNode::new("Input", NodeDummy));
    let out1 = scenery.add_node(OpticNode::new("Output", NodeDummy));
    let bs1 = scenery.add_node(OpticNode::new("Beamsplitter 1", NodeBeamSplitter));
    let bs2 = scenery.add_node(OpticNode::new("Beamsplitter 2", NodeBeamSplitter));
    let m1 = scenery.add_node(OpticNode::new("Mirror 1", NodeDummy));
    let m2 = scenery.add_node(OpticNode::new("Mirror 2", NodeDummy));
    
    scenery.connect_nodes(in1, "rear", bs1, "input1").unwrap();
    scenery.connect_nodes(bs1, "out1_trans1_refl2", m1, "front").unwrap();
    scenery.connect_nodes(bs1, "out2_trans2_refl1", m2, "front").unwrap();

    scenery.connect_nodes(m1, "rear", bs2, "input1").unwrap();
    scenery.connect_nodes(m2, "rear", bs2, "input2").unwrap();
    scenery.connect_nodes(bs2, "out1_trans1_refl2", out1, "front").unwrap();
    scenery.connect_nodes(bs2, "out2_trans2_refl1", out1, "front").unwrap();


    let path = "graph_w_ports.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot_w_ports()).unwrap();
}
