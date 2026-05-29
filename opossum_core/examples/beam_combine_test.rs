#![allow(missing_docs)]
use num::Zero;
use opossum_core::{analyzers::energy::EnergyConfig, prelude::*};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("beam combiner demo");
    let i_s1 = scenery.add_node(SourcePort::new("Source 633nm"))?;
    let i_s2 = scenery.add_node(SourcePort::new("Source 1053nm"))?;
    let i_bs = scenery
        .add_node(BeamSplitter::new("bs", &SplittingConfigBuilder::FixedRatio(0.5))?)?;
    let i_spec = scenery.add_node(Spectrometer::default())?;

    scenery.connect_nodes(i_s1, "output_1", i_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(i_s2, "output_1", i_bs, "input_2", Length::zero())?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_spec, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);

    let energy_data_builder_1 = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    )?);
    let energy_data_builder_2 = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![(nanometer!(635.0), joule!(1.0))],
        nanometer!(1.0),
    )?);
    let mut energy_config = EnergyConfig::default();
    energy_config.map_source(i_s1, energy_data_builder_1.into());
    energy_config.map_source(i_s2, energy_data_builder_2.into());
    doc.add_analyzer(AnalyzerType::Energy(energy_config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/beam_combiner_test.opm",
    ))
}
