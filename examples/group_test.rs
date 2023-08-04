use opossum::analyzer::AnalyzerEnergy;
use opossum::error::OpossumError;
use opossum::nodes::{Dummy, NodeReference, NodeGroup, BeamSplitter};
use opossum::optic_scenery::OpticScenery;
use uom::typenum::N3;
use std::fs::File;
use std::io::Write;
use opossum::optic_node::{OpticNode};

fn main() -> Result<(), OpossumError> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Node Group test section".into());

    // let n1 = scenery.add_element("TFP_no_group", Dummy);
    let mut group1 = NodeGroup::new();
    group1.expand_view(true);
    let g1_n1 = group1.add_node(OpticNode::new("TFP1_g1", Dummy));
    let g1_n2 = group1.add_node(OpticNode::new("TFP2_g1", BeamSplitter::default()));
    group1.link_ports("out1", g1_n2, "out1_trans1_refl2");
    group1.link_ports("out1", g1_n2, "out2_trans2_refl1");
    group1.connect_nodes(g1_n1, "rear", g1_n2, "input1").unwrap();

    let mut nested_group = NodeGroup::new(); 
    let nested_g_n1 = nested_group.add_node(OpticNode::new("TFP1_g_nested_1", Dummy));
    let nested_g_n2 = nested_group.add_node(OpticNode::new("TFP1_g_nested_2", Dummy));
    nested_group.expand_view(false);

    nested_group.connect_nodes(nested_g_n1, "rear", nested_g_n2, "front").unwrap();
    nested_group.link_ports("in1", nested_g_n1, "front");
    nested_group.link_ports("out1", nested_g_n2, "rear");
    
    let nested_group_index = group1.add_node(OpticNode::new("nested_group", nested_group));
    group1.connect_nodes(nested_group_index, "out1", g1_n1, "front").unwrap();

    
    let mut group2: NodeGroup = NodeGroup::new();
    group2.expand_view(true);
    let g2_n1 = group2.add_node(OpticNode::new("TFP1_g2", Dummy));
    let g2_n2 = group2.add_node(OpticNode::new("TFP2_g2", Dummy));
    group2.link_ports("in1", g2_n1, "front");
    group2.connect_nodes(g2_n1, "rear", g2_n2, "front").unwrap();


    let scene_g1 = scenery.add_node(OpticNode::new("Group1_TFPs", group1));
    let scene_g2 = scenery.add_node(OpticNode::new("Group2_TFPs", group2));
    // set_output_port
    scenery.connect_nodes(scene_g1, "out1", scene_g2, "in1");
    let path = "graph_group.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()?).unwrap();

    Ok(())
}
