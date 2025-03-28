#![allow(missing_docs)]
use std::path::Path;

use num::Zero;
use opossum::{
    analyzers::AnalyzerType,
    error::OpmResult,
    joule,
    lightdata::{energy_spectrum_builder::EnergyDataBuilder, light_data_builder::LightDataBuilder},
    nanometer,
    nodes::{BeamSplitter, Dummy, FilterType, IdealFilter, NodeGroup, Source},
    ray::SplittingConfig,
    spectrum_helper::{self, generate_filter_spectrum},
    OpmDocument,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("beam combiner demo");
    let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    ));
    let i_s1 = scenery.add_node(Source::new("Source 1", light_data_builder))?;
    let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
        vec![(nanometer!(1053.0), joule!(1.0))],
        nanometer!(1.0),
    ));
    let i_s2 = scenery.add_node(Source::new("Source 2", light_data_builder))?;
    let i_bs = scenery.add_node(BeamSplitter::new("bs", &SplittingConfig::Ratio(0.5)).unwrap())?;
    let filter_spectrum = generate_filter_spectrum(
        nanometer!(400.0)..nanometer!(1100.0),
        nanometer!(1.0),
        &spectrum_helper::FilterType::LongPassStep {
            cut_off: nanometer!(700.0),
        },
    )?;
    let i_f = scenery.add_node(IdealFilter::new(
        "filter",
        &FilterType::Spectrum(filter_spectrum),
    )?)?;
    let i_d1 = scenery.add_node(Dummy::default())?;

    scenery.connect_nodes(i_s1, "output_1", i_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(i_s2, "output_1", i_bs, "input_2", Length::zero())?;
    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_f, "input_1", Length::zero())?;
    scenery.connect_nodes(i_f, "output_1", i_d1, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/beam_combiner_test.opm"))
}
