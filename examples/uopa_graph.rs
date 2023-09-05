use opossum::analyzer::AnalyzerEnergy;
use opossum::error::OpossumError;
use opossum::nodes::{BeamSplitter, Dummy};
use opossum::OpticScenery;

use std::fs::File;
use std::io::Write;

fn main() -> Result<(), OpossumError> {
    println!("PHELIX uOPA opticscenery example");
    let mut scenery = OpticScenery::new();

    scenery.set_description("PHELIX uOPA");
    println!("default opticscenery: {:?}", scenery);
    println!("export to `dot` format: {}", scenery.to_dot()?);

    let pulse_generation_split_node = scenery.add_element("Pulse Generation", Dummy::default());
    let u_opa_1_node = scenery.add_element("uOPA Stage 1", Dummy::default());
    let u_opa_2_node = scenery.add_element("uOPA Stage 2", Dummy::default());
    let pump_pre_amplifier_node = scenery.add_element("Pump Pre-Amplifier", Dummy::default());
    let pump_main_amplifier_node = scenery.add_element("Pump Main-Amplifier", Dummy::default());
    let pump_compressor_node = scenery.add_element("Pump Compressor", Dummy::default());
    let pump_shg_node = scenery.add_element("Pump SHG", Dummy::default());
    let pump_splitter_node = scenery.add_element("Pump Beam Splitter", BeamSplitter::default());

    scenery.connect_nodes(pulse_generation_split_node, "rear", u_opa_1_node, "front")?;
    // scenery
    //     .connect_nodes(
    //         pulse_generation_split_node,
    //         "rear",
    //         pump_pre_amplifier_node,
    //         "front",
    //     )
    //     .unwrap();
    scenery.connect_nodes(
        pump_pre_amplifier_node,
        "rear",
        pump_main_amplifier_node,
        "front",
    )?;
    scenery.connect_nodes(
        pump_main_amplifier_node,
        "rear",
        pump_compressor_node,
        "front",
    )?;
    scenery.connect_nodes(pump_compressor_node, "rear", pump_shg_node, "front")?;
    scenery.connect_nodes(pump_shg_node, "rear", pump_splitter_node, "input1")?;
    scenery.connect_nodes(
        pump_splitter_node,
        "out1_trans1_refl2",
        u_opa_1_node,
        "front",
    )?;
    scenery.connect_nodes(u_opa_1_node, "rear", u_opa_2_node, "front")?;
    scenery.connect_nodes(
        pump_splitter_node,
        "out2_trans2_refl1",
        u_opa_2_node,
        "front",
    )?;

    let mut scenery_2 = OpticScenery::new();
    scenery_2.set_description("PHELIX uOPA Pump Pre-Amplifier".into());

    let spm_node = scenery_2.add_element("SPM", Dummy::default());
    let circ1_node = scenery_2.add_element("Circulator Port 1", Dummy::default());
    let circ2_node = scenery_2.add_element("Circulator Port 2", Dummy::default());
    let circ3_node = scenery_2.add_element("Circulator Port 3", Dummy::default());
    let cfbg_node = scenery_2.add_element("CFBG", Dummy::default());
    let isolator1_node = scenery_2.add_element("FI", Dummy::default());
    let tap1_node = scenery_2.add_element("Tap", Dummy::default());
    let diode1_node = scenery_2.add_element("Laser Diode", Dummy::default());
    let wdm_node = scenery_2.add_element("WDM", Dummy::default());
    let yb_fiber1_node = scenery_2.add_element("Yb-Fiber 1", Dummy::default());
    let tap2_node = scenery_2.add_element("Tap", Dummy::default());
    let aom_node = scenery_2.add_element("AOM", Dummy::default());
    let isolator2_node = scenery_2.add_element("FI", Dummy::default());
    let yb_fiber2_node_node = scenery_2.add_element("Yb-Fiber 2", Dummy::default());
    let dichroic_node = scenery_2.add_element("DCM", Dummy::default());
    let diode2_node = scenery_2.add_element("Laser Diode", Dummy::default());
    // let monitor1_node = scenery_2.add_element("Monitor", Dummy);
    let monitor2_node = scenery_2.add_element("Monitor", Dummy::default());
    let monitor3_node = scenery_2.add_element("Monitor", Dummy::default());

    scenery_2.connect_nodes(spm_node, "rear", circ1_node, "front")?;
    scenery_2.connect_nodes(circ1_node, "rear", circ2_node, "front")?;
    scenery_2.connect_nodes(circ2_node, "rear", cfbg_node, "front")?;
    scenery_2.connect_nodes(cfbg_node, "rear", circ3_node, "front")?;
    // scenery_2.connect_nodes(cfbg_node, "rear", monitor1_node, "front")?;
    scenery_2.connect_nodes(circ3_node, "rear", isolator1_node, "front")?;
    scenery_2.connect_nodes(isolator1_node, "rear", tap1_node, "front")?;
    scenery_2.connect_nodes(tap1_node, "rear", monitor2_node, "front")?;
    // scenery_2.connect_nodes(tap1_node, "rear", wdm_node, "front")?;
    scenery_2.connect_nodes(diode1_node, "rear", wdm_node, "front")?;
    scenery_2.connect_nodes(wdm_node, "rear", yb_fiber1_node, "front")?;
    scenery_2.connect_nodes(yb_fiber1_node, "rear", tap2_node, "front")?;
    scenery_2.connect_nodes(tap2_node, "rear", monitor3_node, "front")?;
    // scenery_2.connect_nodes(tap2_node, "rear", aom_node, "front")?;
    scenery_2.connect_nodes(aom_node, "rear", isolator2_node, "front")?;
    scenery_2.connect_nodes(isolator2_node, "rear", yb_fiber2_node_node, "front")?;
    scenery_2.connect_nodes(yb_fiber2_node_node, "rear", dichroic_node, "front")?;
    // scenery_2.connect_nodes(dichroic_node, "rear", dichroic_node, "front")?;
    scenery_2.connect_nodes(diode2_node, "rear", dichroic_node, "front")?;

    let mut scenery_3 = OpticScenery::new();
    scenery_3.set_description("PHELIX uOPA Pump Regenerative Main-Amplifier".into());

    let _pol1_node = scenery_2.add_element("Picker Polarizer", Dummy::default());
    let _pc1_node = scenery_2.add_element("Pulse Picker PC", Dummy::default());
    let _pol2_node = scenery_2.add_element("Cavity Polarizer", Dummy::default());
    let _yb_yag_node = scenery_2.add_element("Yb:YAG", Dummy::default());
    let _pc2_node = scenery_2.add_element("Cavity PC", Dummy::default());
    let _qwp_node = scenery_2.add_element("Quarter Waveplate", Dummy::default());
    let _mirror1_node = scenery_2.add_element("Curved Mirror 1", Dummy::default());
    let _mirror2_node = scenery_2.add_element("Curved Mirror 1", Dummy::default());
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
    write!(output, "{}", scenery.to_dot()?).unwrap();

    let path = "uOPA_PreAmp.dot";
    let mut output = File::create(path).unwrap();
    write!(output, "{}", scenery_2.to_dot()?).unwrap();

    let mut analyzer = AnalyzerEnergy::new(&scenery);
    analyzer.analyze()?;

    Ok(())
}
