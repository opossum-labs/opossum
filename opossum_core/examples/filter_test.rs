use num::Zero;
use opossum_core::{analyzers::energy::EnergyConfig, prelude::*};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("filter system demo");
    let i_src = scenery.add_node(SourcePort::new("Source"))?;

    let i_bs = scenery
        .add_node(BeamSplitter::new("bs", &SplittingConfigBuilder::FixedRatio(0.6)).unwrap())?;

    let i_f = scenery.add_node(IdealFilter::new(
        "filter",
        &FilterTypeBuilder::Spectrum(SpectralFilterBuilder::FromFile(
            Path::new("./opossum_core/files_for_testing/spectrum/NF633-25.csv").to_path_buf(),
        )),
    )?)?;

    let i_d1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum_core::nodes::Metertype::IdealEnergyMeter,
    ))?;
    let i_d2 = scenery.add_node(Spectrometer::default())?;
    let i_d3 = scenery.add_node(EnergyMeter::new(
        "Energy meter 2",
        opossum_core::nodes::Metertype::IdealEnergyMeter,
    ))?;

    scenery.connect_nodes(i_src, "output_1", i_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "input_1", Length::zero())?;
    scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_f, "input_1", Length::zero())?;
    scenery.connect_nodes(i_f, "output_1", i_d2, "input_1", Length::zero())?;
    scenery.connect_nodes(i_d2, "output_1", i_d3, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);

    let mut config = EnergyConfig::default();
    let energy_data_builder = EnergyDataBuilder::Raw(
        BandFilter::new(
            BandFilterType::BandPass,
            nanometer!(630.),
            nanometer!(50.),
            (0.)..(1.),
            Some(nanometer!(25.)),
            nanometer!(560.)..nanometer!(700.),
            nanometer!(0.01),
        )?
        .into(),
    );
    config.map_source(i_src, energy_data_builder);
    doc.add_analyzer(AnalyzerType::Energy(config));
    doc.save_to_file(Path::new("./opossum_core/playground/filter_test.opm"))
}
