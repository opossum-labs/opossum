//! Two-stage amplifier chain with constant gain, analyzed at two operating points.
//!
//! This is the code counterpart of the "Model an amplifier" how-to guide in the handbook: the same
//! set-up a user clicks together in the GUI, written out as a program. Running it writes an `.opm`
//! file that can be opened in the GUI, where both scenarios show up in the "Pump scenarios" panel
//! exactly as configured here.
//!
//! What makes a component an amplifier is *not* a node type of its own: it is a node enclosing a
//! volume of material (a lens, a wedge or a cylindric lens) that a pump scenario assigns a
//! [`GainModel`] to. The scenario lives on the document rather than on the node, so the very same
//! model can be run at several operating points without being edited in between.
//!
//! # What to look for
//!
//! Open the resulting `.opm` file in the GUI and run the energy-flow analysis. The energy meter at
//! the end of the chain reads the product of the two gain factors — 20 J at full power (5 × 4),
//! 5 J at half power (2.5 × 2) — starting from a 1 J input. Two reports are produced, one per
//! scenario, named after their scenario.
//!
//! Run with
//!
//! ```bash
//! cargo run -p opossum_core --example amplifier_const_gain_chain
//! ```
use opossum_core::{
    gain::{ConstGain, GainModel},
    prelude::*,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Amplifier chain demo");

    // The hardware: what the components *are* belongs to the model.
    let source = scenery.add_node(SourcePort::new("oscillator"))?;
    let first_head = scenery.add_node(Lens::new(
        "amplifier head 1",
        millimeter!(f64::INFINITY),
        millimeter!(f64::INFINITY),
        millimeter!(20.0),
        RefrIndexConst::new(1.5)?,
    )?)?;
    let second_head = scenery.add_node(Lens::new(
        "amplifier head 2",
        millimeter!(f64::INFINITY),
        millimeter!(f64::INFINITY),
        millimeter!(20.0),
        RefrIndexConst::new(1.5)?,
    )?)?;
    let meter = scenery.add_node(EnergyMeter::new(
        "output energy",
        Metertype::IdealEnergyMeter,
    )?)?;
    scenery.connect_nodes(source, "output_1", first_head, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(
        first_head,
        "output_1",
        second_head,
        "input_1",
        millimeter!(100.0),
    )?;
    scenery.connect_nodes(second_head, "output_1", meter, "input_1", millimeter!(50.0))?;

    let mut document = OpmDocument::new(scenery);

    // Marking a node as an amplifier is bookkeeping for the user interface - it is what makes the
    // GUI offer the node a row in the scenario editor. The analysis does not read it: what a node
    // does is decided by the gain model set below.
    document.set_is_amplifier_node(first_head, true);
    document.set_is_amplifier_node(second_head, true);

    // The operating points: how hard each head is driven belongs to the run, not to the model. Two
    // scenarios, so one analysis yields two reports of the same chain.
    for (name, first_gain, second_gain) in [("full power", 5.0, 4.0), ("half power", 2.5, 2.0)] {
        let scenario_id = document.add_pump_scenario(name);
        let scenario = document
            .pump_scenario_mut(scenario_id)
            .expect("the scenario just added must be there");
        scenario.set_gain_model(first_head, GainModel::Const(ConstGain::new(first_gain)?));
        scenario.set_gain_model(second_head, GainModel::Const(ConstGain::new(second_gain)?));
    }

    let mut config = EnergyConfig::default();
    config.map_source(
        source,
        EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
            vec![(nanometer!(1054.0), joule!(1.0))],
            nanometer!(1.0),
        )?),
    );
    let analyzer_id = document.add_analyzer(AnalyzerType::Energy(config));

    // Nothing is amplified until an analyzer is told which operating points to run in. Without this
    // the model is analyzed once, passively - which is what every analyzer did before scenarios
    // existed.
    let scenario_ids = document.pump_scenarios().keys().copied().collect();
    document
        .analyzer_mut(analyzer_id)
        .expect("the analyzer just added must be there")
        .set_pump_scenarios(scenario_ids);

    // 1 J in, so the meter reads the chain's overall gain directly: 20 J at full power, 5 J at half.
    document.save_to_file(Path::new(
        "./opossum_core/playground/amplifier_const_gain_chain.opm",
    ))
}
