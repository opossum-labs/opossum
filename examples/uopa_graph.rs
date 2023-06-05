use opossum::optic_scenery::OpticScenery;
use opossum::optic_node::OpticNode;
use opossum::nodes::NodeDummy;

use std::fs::File;
use std::io::Write;

fn main() {
    println!("PHELIX uOPA opticscenery example");
    let mut scenery = OpticScenery::new();

    scenery.set_description("PHELIX uOPA".into());
    println!("default opticscenery: {:?}", scenery);
    println!("export to `dot` format: {}", scenery.to_dot());

    let pulse_generation_split_node   =scenery.add_node(OpticNode::new("Pulse Generation", Box::new(NodeDummy)));
    let uOPA_1_node   =scenery.add_node(OpticNode::new("uOPA Stage 1", Box::new(NodeDummy)));
    let uOPA_2_node   =scenery.add_node(OpticNode::new("uOPA Stage 2", Box::new(NodeDummy)));
    let pump_pre_amplifier_node =scenery.add_node(OpticNode::new("Pump Pre-Amplifier", Box::new(NodeDummy)));
    let pump_main_amplifier_node =scenery.add_node(OpticNode::new("Pump Main-Amplifier", Box::new(NodeDummy)));
    let pump_compressor_node =scenery.add_node(OpticNode::new("Pump Compressor", Box::new(NodeDummy)));
    let pump_shg_node =scenery.add_node(OpticNode::new("Pump SHG", Box::new(NodeDummy)));
    let pump_splitter_node   =scenery.add_node(OpticNode::new("Pump Beam Splitter", Box::new(NodeDummy)));

    scenery.connect_nodes(pulse_generation_split_node, uOPA_1_node);
    scenery.connect_nodes(pulse_generation_split_node, pump_pre_amplifier_node);
    scenery.connect_nodes(pump_pre_amplifier_node, pump_main_amplifier_node);
    scenery.connect_nodes(pump_main_amplifier_node, pump_compressor_node);
    scenery.connect_nodes(pump_compressor_node, pump_shg_node);
    scenery.connect_nodes(pump_shg_node, pump_splitter_node);
    scenery.connect_nodes(pump_splitter_node, uOPA_1_node);
    scenery.connect_nodes(uOPA_1_node, uOPA_2_node);
    scenery.connect_nodes(pump_splitter_node, uOPA_2_node);
    
    let mut scenery_2 = OpticScenery::new();

    scenery_2.set_description("PHELIX uOPA Pump Pre-Amplifier".into());

    let spm_node            = scenery_2.add_node(OpticNode::new("SPM", Box::new(NodeDummy)));
    let circ_node           = scenery_2.add_node(OpticNode::new("Circulator", Box::new(NodeDummy)));
    let cfbg_node           = scenery_2.add_node(OpticNode::new("CFBG", Box::new(NodeDummy)));
    let isolator1_node      = scenery_2.add_node(OpticNode::new("FI", Box::new(NodeDummy)));
    let tap1_node           = scenery_2.add_node(OpticNode::new("Tap", Box::new(NodeDummy)));
    let diode1_node         = scenery_2.add_node(OpticNode::new("Laser Diode", Box::new(NodeDummy)));
    let wdm_node            = scenery_2.add_node(OpticNode::new("WDM", Box::new(NodeDummy)));
    let yb_fiber1_node      = scenery_2.add_node(OpticNode::new("Yb-Fiber 1", Box::new(NodeDummy)));
    let tap2_node           = scenery_2.add_node(OpticNode::new("Tap", Box::new(NodeDummy)));
    let aom_node            = scenery_2.add_node(OpticNode::new("AOM", Box::new(NodeDummy)));
    let isolator2_node      = scenery_2.add_node(OpticNode::new("FI", Box::new(NodeDummy)));
    let yb_fiber2_node_node = scenery_2.add_node(OpticNode::new("Yb-Fiber 2", Box::new(NodeDummy)));
    let dichroic_node       = scenery_2.add_node(OpticNode::new("DCM", Box::new(NodeDummy)));
    let diode2_node         = scenery_2.add_node(OpticNode::new("Laser Diode", Box::new(NodeDummy)));

    scenery_2.connect_nodes(spm_node, circ_node);
    scenery_2.connect_nodes(circ_node, cfbg_node);
    // scenery_2.connect_nodes(spm_node, circ_node);

    // let mira_node1          =scenery.add_node(OpticNode::new("Mira", Box::new(NodeDummy)));
    // let qwp_node1           =scenery.add_node(OpticNode::new("Quarter Wave Plate", Box::new(NodeDummy)));
    // let pol_node1           =scenery.add_node(OpticNode::new("Polarizer", Box::new(NodeDummy)));
    
    
    // let dichroic_node1      =scenery.add_node(OpticNode::new("Dichroic Mirror", Box::new(NodeDummy)));
    // let fiber_amp_node1     =scenery.add_node(OpticNode::new("Pump Fiber Amplifier", Box::new(NodeDummy)));
    // let periscope_node1     =scenery.add_node(OpticNode::new("Periscope", Box::new(NodeDummy)));
    // let kepler_tel_node1    =scenery.add_node(OpticNode::new("Kepler Telescope", Box::new(NodeDummy)));
    // let pol_node2           =scenery.add_node(OpticNode::new("Pulse Picker Polarizer", Box::new(NodeDummy)));
    // let pol_node3           =scenery.add_node(OpticNode::new("Pulse Picker Polarizer", Box::new(NodeDummy)));
    // let pockels_cell_node1  =scenery.add_node(OpticNode::new("Pulse Picker Pockels Cell 1", Box::new(NodeDummy)));
    // let pockels_cell_node2  =scenery.add_node(OpticNode::new("Pulse Picker Pockels Cell 1", Box::new(NodeDummy)));
    // let regen_amp_node1     =scenery.add_node(OpticNode::new("Regenerative Amplifier", Box::new(NodeDummy)));
    // let galilei_node1       =scenery.add_node(OpticNode::new("Galilei Telescope", Box::new(NodeDummy)));
    // let compressor_node1    =scenery.add_node(OpticNode::new("Compressor", Box::new(NodeDummy)));
    // let galilei_node2       =scenery.add_node(OpticNode::new("Galilei Telescope", Box::new(NodeDummy)));
    // let delay_node1         =scenery.add_node(OpticNode::new("Delay Stage", Box::new(NodeDummy)));
    // let shg_node1           =scenery.add_node(OpticNode::new("SHG", Box::new(NodeDummy)));
    // let hwp_node1           =scenery.add_node(OpticNode::new("Half Wave Plate", Box::new(NodeDummy)));    
    // let pump_bs_node1       =scenery.add_node(OpticNode::new("Polarizing Beam Splitter", Box::new(NodeDummy)));
    // let pump_kepler_node1   =scenery.add_node(OpticNode::new("Kepler Telescope", Box::new(NodeDummy)));


    // scenery.connect_nodes(mira_node1, qwp_node1);
    // scenery.connect_nodes(qwp_node1, pol_node1);
    // scenery.connect_nodes(pol_node1, dichroic_node1);
    // scenery.connect_nodes(pol_node1, fiber_amp_node1);

    // scenery.connect_nodes(fiber_amp_node1, periscope_node1);
    // scenery.connect_nodes(periscope_node1, kepler_tel_node1);
    // scenery.connect_nodes(kepler_tel_node1, pol_node2);
    // scenery.connect_nodes(pol_node2, pockels_cell_node1);
    // scenery.connect_nodes(pockels_cell_node1, regen_amp_node1);
    // scenery.connect_nodes(regen_amp_node1, pockels_cell_node2);

    // scenery.connect_nodes(pockels_cell_node2, pol_node3);
    // scenery.connect_nodes(pol_node3, galilei_node1);
    // scenery.connect_nodes(galilei_node1, compressor_node1);
    // scenery.connect_nodes(compressor_node1, galilei_node2);
    // scenery.connect_nodes(galilei_node2, delay_node1);
    // scenery.connect_nodes(delay_node1, shg_node1);
    // scenery.connect_nodes(shg_node1, hwp_node1);
    // scenery.connect_nodes(hwp_node1, pump_bs_node1);

    // scenery.connect_nodes(pump_bs_node1, pump_kepler_node1);
    // scenery.connect_nodes(pump_kepler_node1, dichroic_node1);

    let path = "uOPA.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}\n{}", scenery.to_dot()).unwrap();
}