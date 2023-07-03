use opossum::nodes::{NodeDummy, NodeGroup, NodeReference};
use opossum::optic_node::OpticNode;
use opossum::optic_scenery::OpticScenery;
use opossum::analyzer::AnalyzerEnergy;
use std::fs::File;
use std::io::Write;

fn main() {
    let mut scenery = OpticScenery::new();
    scenery.set_description("PreAmp Doublepass section".into());
    let n1 = scenery.add_element("TFP", NodeDummy);
    let n2 = scenery.add_element("19mm amp", NodeDummy);
    let n3 = scenery.add_element("Faraday", NodeDummy);
    let n4 = scenery.add_element("0° mirror", NodeDummy);

    let ref_node= NodeReference::new(scenery.node_ref(n1).unwrap());
    let mut node = OpticNode::new("Ref", ref_node);
    node.set_inverted(true);
    let n1r=scenery.add_node(node);
    
    let ref_node= NodeReference::new(scenery.node_ref(n3).unwrap());
    let mut node = OpticNode::new("Ref", ref_node);
    node.set_inverted(true);
    let n3r = scenery.add_node(node);

    let ref_node= NodeReference::new(scenery.node_ref(n2).unwrap());
    let mut node = OpticNode::new("Ref", ref_node);
    node.set_inverted(true);
    let n2r = scenery.add_node(node);

    scenery.connect_nodes(n1, "rear", n2, "front").unwrap();
    scenery.connect_nodes(n2, "rear", n3, "front").unwrap();
    scenery.connect_nodes(n3, "rear", n4, "front").unwrap();
    scenery.connect_nodes(n4, "rear", n3r, "rear").unwrap();
    scenery.connect_nodes(n3r, "front", n2r, "rear").unwrap();
    scenery.connect_nodes(n2r, "front", n1r, "rear").unwrap();

    let mut group = NodeGroup::new();
    let g_n1 = group.add_node(OpticNode::new("Beamsplitter", NodeDummy));
    let g_n2 = group.add_node(OpticNode::new("Lens", NodeDummy));
    let g_n3 = group.add_node(OpticNode::new("Lens2", NodeDummy));
    group.connect_nodes(g_n1, "rear", g_n2, "front").unwrap();
    group.connect_nodes(g_n2, "rear", g_n3, "front").unwrap();
    scenery.add_node(OpticNode::new("CamBox", group));
    let path = "graph.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
    // write!(output, "{}", scenery.to_dot()).unwrap();

    let analyzer=AnalyzerEnergy::new(&scenery);
    analyzer.analyze();
}
