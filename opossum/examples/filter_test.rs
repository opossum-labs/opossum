use std::path::Path;

use num::Zero;
use opossum::{
    analyzers::AnalyzerType, error::OpmResult, joule, lightdata::{
        energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
        light_data_builder::LightDataBuilder,
    }, nanometer, nodes::{ideal_filter::{BandFilter, BandFilterType, EdgeFilter, EdgeFilterType, FilterTypeBuilder, SpectralFilterBuilder}, BeamSplitter, EnergyMeter, FilterType, IdealFilter, NodeGroup, Source, Spectrometer}, ray::SplittingConfig, spectrum::Spectrum, OpmDocument
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("filter system demo");
    // let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
    //     EnergyLaserLines::new(vec![(nanometer!(633.0), joule!(1.0))], nanometer!(1.0))?,
    // ));
    // let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::Raw(
    //     BandFilter::new(BandFilterType::BandPass, nanometer!(630.), nanometer!(10.), Some(nanometer!(2.)), nanometer!(620.)..nanometer!(640.), nanometer!(0.01))?.into()
    // ));
    let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::Raw(
        BandFilter::new(BandFilterType::BandPass, nanometer!(630.), nanometer!(20.), Some(nanometer!(300.)), nanometer!(600.)..nanometer!(660.), nanometer!(0.01))?.into()
    ));
    let i_s = scenery.add_node(Source::new("Source", light_data_builder))?;
    let i_bs = scenery.add_node(BeamSplitter::new("bs", &SplittingConfig::Ratio(0.6)).unwrap())?;
    // let filter_spectrum =
    //     Spectrum::from_csv(Path::new("./opossum/files_for_testing/spectrum/NE03B.csv"))?;
    // let i_f = scenery.add_node(IdealFilter::new(
    //     "filter",
    //     &FilterTypeBuilder::Spectrum(SpectralFilterBuilder::FromFile(Path::new("./opossum/files_for_testing/spectrum/NE03B.csv").to_path_buf())),
    // )?)?;
    // let i_f = scenery.add_node(IdealFilter::new(
    //     "filter",
    //     &FilterTypeBuilder::Spectrum(SpectralFilterBuilder::EdgeFilter(EdgeFilter::new(EdgeFilterType::ShortPass, nanometer!(630.), Some(nanometer!(5.)), nanometer!(500.)..nanometer!(1000.), nanometer!(0.5))?)),
    // )?)?;
    let i_f = scenery.add_node(IdealFilter::new(
        "filter",
        &FilterTypeBuilder::Constant(1.),
    )?)?;
    let i_d1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ))?;
    let i_d2 = scenery.add_node(Spectrometer::default())?;
    let i_d3 = scenery.add_node(EnergyMeter::new(
        "Energy meter 2",
        opossum::nodes::Metertype::IdealEnergyMeter,
    ))?;

    scenery.connect_nodes(i_s, "output_1", i_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "input_1", Length::zero())?;
    scenery.connect_nodes(i_bs, "out2_trans2_refl1", i_f, "input_1", Length::zero())?;
    scenery.connect_nodes(i_f, "output_1", i_d2, "input_1", Length::zero())?;
    scenery.connect_nodes(i_d2, "output_1", i_d3, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/filter_test.opm"))
}
