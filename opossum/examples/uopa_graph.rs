use opossum::error::OpmResult;
use opossum::nodes::{BeamSplitter, Dummy};
use opossum::OpticScenery;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();

    scenery.set_description("PHELIX uOPA")?;

    let pulse_generation_split_node = scenery.add_node(Dummy::new("Pulse Generation"));
    let u_opa_1_node = scenery.add_node(Dummy::new("uOPA Stage 1"));
    let u_opa_2_node = scenery.add_node(Dummy::new("uOPA Stage 2"));
    let pump_pre_amplifier_node = scenery.add_node(Dummy::new("Pump Pre-Amplifier"));
    let pump_main_amplifier_node = scenery.add_node(Dummy::new("Pump Main-Amplifier"));
    let pump_compressor_node = scenery.add_node(Dummy::new("Pump Compressor"));
    let pump_shg_node = scenery.add_node(Dummy::new("Pump SHG"));
    let pump_splitter_node = scenery.add_node(BeamSplitter::default()); // Pump Beam Splitter

    scenery.connect_nodes(pulse_generation_split_node, "rear", u_opa_1_node, "front")?;
    scenery
        .connect_nodes(
            pulse_generation_split_node,
            "rear",
            pump_pre_amplifier_node,
            "front",
        )
        .unwrap();
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
    scenery_2.set_description("PHELIX uOPA Pump Pre-Amplifier".into())?;

    let spm_node = scenery_2.add_node(Dummy::new("SPM"));
    let circ1_node = scenery_2.add_node(Dummy::new("Circulator Port1"));
    let circ2_node = scenery_2.add_node(Dummy::new("Circulator Port 2"));
    let circ3_node = scenery_2.add_node(Dummy::new("Circulator Port 3"));
    let cfbg_node = scenery_2.add_node(Dummy::new("CFBG"));
    let isolator1_node = scenery_2.add_node(Dummy::new("FI"));
    let tap1_node = scenery_2.add_node(Dummy::new("Tap"));
    let diode1_node = scenery_2.add_node(Dummy::new("LaserDiode"));
    let wdm_node = scenery_2.add_node(Dummy::new("WDM"));
    let yb_fiber1_node = scenery_2.add_node(Dummy::new("Yb-Fiber"));
    let tap2_node = scenery_2.add_node(Dummy::new("Tap"));
    let aom_node = scenery_2.add_node(Dummy::new("AOM"));
    let isolator2_node = scenery_2.add_node(Dummy::new("FI"));
    let yb_fiber2_node_node = scenery_2.add_node(Dummy::new("Yb-Fiber 2"));
    let dichroic_node = scenery_2.add_node(Dummy::new("DCM"));
    let diode2_node = scenery_2.add_node(Dummy::new("Laser Diode"));
    // let monitor1_node = scenery_2.add_element("Monitor", Dummy);
    let monitor2_node = scenery_2.add_node(Dummy::new("Monitor"));
    let monitor3_node = scenery_2.add_node(Dummy::new("Monitor"));

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
    scenery_3.set_description("PHELIX uOPA Pump Regenerative Main-Amplifier".into())?;

    let _pol1_node = scenery_2.add_node(Dummy::new("Picker Polarizer"));
    let _pc1_node = scenery_2.add_node(Dummy::new("Pulse Picker PC"));
    let _pol2_node = scenery_2.add_node(Dummy::new("Cavity Polarizer"));
    let _yb_yag_node = scenery_2.add_node(Dummy::new("Yb:YAG"));
    let _pc2_node = scenery_2.add_node(Dummy::new("Cavity PC"));
    let _qwp_node = scenery_2.add_node(Dummy::new("Quarter Waveplate"));
    let _mirror1_node = scenery_2.add_node(Dummy::new("Curved Mirror 1"));
    let _mirror2_node = scenery_2.add_node(Dummy::new("Curved Mirror 1"));
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

    scenery.save_to_file(Path::new("./opossum/playground/uOPA.opm"))?;
    scenery_2.save_to_file(Path::new("./opossum/playground/uOPA_PreAmp.opm"))?;
    Ok(())
}
