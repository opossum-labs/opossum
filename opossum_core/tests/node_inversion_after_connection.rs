use opossum_core::{analyzers::energy::EnergyConfig, prelude::*, utils::LockExt};

/// This test creates a simple optical setup with a source, a dummy node, and an energy meter.
/// The dummy nodes is then inverted after already being connected in the setup. The test checks if the inversion
/// is correctly applied and if the energy meter can still measure the energy after the inversion.
///
/// CURRENTLY THIS IS A FAILING TEST!
///
/// A solution incorporates a general change in the handling of node connections.
/// See issue [`#990`](https://github.com/opossum-labs/opossum/issues/990).
#[test]
#[ignore]
fn node_inversion_after_connection() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Node inversion test");
    let i_src = scenery.add_node(SourcePort::default())?;
    let i_node = scenery.add_node(Dummy::default())?;
    let i_energy_meter = scenery.add_node(EnergyMeter::default())?;

    // Connect the nodes in a simple chain: Source -> Dummy -> Energy Meter
    scenery.connect_nodes(i_src, "output_1", i_node, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(
        i_node,
        "output_1",
        i_energy_meter,
        "input_1",
        millimeter!(0.0),
    )?;

    // Invert the dummy node AFTER it has been connected in the setup
    let dummy_ref = scenery.node(i_node)?;
    dummy_ref.optical_ref.lock_opm()?.set_inverted(true)?;

    let mut doc = OpmDocument::new(scenery);

    let mut energy_config = EnergyConfig::default();
    energy_config.map_source(
        i_src,
        EnergyDataBuilder::LaserLines(EnergyLaserLines::default()),
    );
    doc.add_analyzer(AnalyzerType::Energy(energy_config));

    let opm_file = doc.to_opm_file_string()?;

    let mut doc = OpmDocument::from_string(&opm_file)?;
    // Analyze here...
    let _reports = doc.analyze()?;
    Ok(())
}
