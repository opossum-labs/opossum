use opossum::optic_scenery::OpticScenery;
use opossum::optic_node::OpticNode;
use opossum::nodes::NodeDummy;

use std::fs::File;
use std::io::Write;

fn main() {
    println!("PHELIX uOPA opticscenery example");
    let mut scenery = OpticScenery::new();

    scenery.set_description("OpticScenery PHELIX uOPA".into());
    println!("default opticscenery: {:?}", scenery);
    println!("export to `dot` format: {}", scenery.to_dot());

    let mira_node1          =scenery.add_node(OpticNode::new("mira1", Box::new(NodeDummy)));
    // let qwp_node1           =scenery.add_node(OpticNode::new("Quarter Waveplate", Box::new(NodeDummy)));
    // let pol_node1           =scenery.add_node(OpticNode::new("Polarizer", Box::new(NodeDummy)));
    // let dichroic_node1      =scenery.add_node(OpticNode::new("Dichroic Mirror", Box::new(NodeDummy)));
    // let fiber_amp_node1     =scenery.add_node(OpticNode::new("Fiber Amplifier", Box::new(NodeDummy)));
    // let periscope_node1     =scenery.add_node(OpticNode::new("Periscope", Box::new(NodeDummy)));
    // let kepler_tel_node1    =scenery.add_node(OpticNode::new("Kepler Telescope", Box::new(NodeDummy)));
    // let pol_node2           =scenery.add_node(OpticNode::new("Polarizer", Box::new(NodeDummy)));
    // let pockels_cell_node1  =scenery.add_node(OpticNode::new("Pockels Cell", Box::new(NodeDummy)));
    // let regen_amp_node1     =scenery.add_node(OpticNode::new("Regenerative Amplifier", Box::new(NodeDummy)));
    // let galilei_node1       =scenery.add_node(OpticNode::new("Galilei Telescope", Box::new(NodeDummy)));
    // let compressor_node1    =scenery.add_node(OpticNode::new("Compressor", Box::new(NodeDummy)));
    // let galilei_node2       =scenery.add_node(OpticNode::new("Galilei Telescope", Box::new(NodeDummy)));
    // let delay_node1         =scenery.add_node(OpticNode::new("Delay Stage", Box::new(NodeDummy)));


    // scenery.connect_nodes(mira_node1, qwp_node1);
    // scenery.connect_nodes(qwp_node1, pol_node1);
    // scenery.connect_nodes(pol_node1, dichroic_node1);
    // scenery.connect_nodes(pol_node1, fiber_amp_node1);

    // scenery.connect_nodes(fiber_amp_node1, periscope_node1);
    // scenery.connect_nodes(periscope_node1, kepler_tel_node1);
    // scenery.connect_nodes(kepler_tel_node1, pol_node2);
    // scenery.connect_nodes(pol_node2, pockels_cell_node1);
    // scenery.connect_nodes(pockels_cell_node1, regen_amp_node1);
    // scenery.connect_nodes(regen_amp_node1, pockels_cell_node1);

    // scenery.connect_nodes(pockels_cell_node1, pol_node2);
    // scenery.connect_nodes(pol_node2, galilei_node1);
    // scenery.connect_nodes(galilei_node1, compressor_node1);
    // scenery.connect_nodes(compressor_node1, galilei_node2);
    // scenery.connect_nodes(galilei_node2, delay_node1);


    let path = "uOPA.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery.to_dot()).unwrap();
}