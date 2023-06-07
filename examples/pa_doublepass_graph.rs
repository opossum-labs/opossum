use opossum::nodes::{NodeDummy, NodeGroup};
use opossum::optic_node::OpticNode;
use opossum::optic_scenery::OpticScenery;

use std::fs::File;
use std::io::Write;

fn main() {
    let mut scenery = OpticScenery::new();
    scenery.set_description("PreAmp Doublepass section".into());
    let n1 = scenery.add_element("TFP", NodeDummy);
    let n2 = scenery.add_element("19mm amp", NodeDummy);
    let n3 = scenery.add_element("Faraday", NodeDummy);
    let n4 = scenery.add_element("0° mirror", NodeDummy);

    // let ref_node= NodeReference::new(scenery.node(n1));
    // let n1r=scenery.add_node("ref", ref_node);

    let mut node = OpticNode::new("Faraday", NodeDummy);
    node.set_inverted(true);
    let n3i = scenery.add_node(node);

    let mut node = OpticNode::new("19mm amp", NodeDummy);
    node.set_inverted(true);
    let n2i = scenery.add_node(node);

    let mut node = OpticNode::new("TFP", NodeDummy);
    node.set_inverted(true);
    let n1i = scenery.add_node(node);

    scenery.connect_nodes(n1, n2).unwrap();
    scenery.connect_nodes(n2, n3).unwrap();
    scenery.connect_nodes(n3, n4).unwrap();
    scenery.connect_nodes(n4, n3i).unwrap();
    scenery.connect_nodes(n3i, n2i).unwrap();
    scenery.connect_nodes(n2i, n1i).unwrap();

    let mut group = NodeGroup::new();
    let g_n1 = group.add_node(OpticNode::new("Beamsplitter", NodeDummy));
    let g_n2 = group.add_node(OpticNode::new("Lens", NodeDummy));
    group.connect_nodes(g_n1, g_n2).unwrap();
    scenery.add_node(OpticNode::new("CamBox", group));
    let path = "graph.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
}
