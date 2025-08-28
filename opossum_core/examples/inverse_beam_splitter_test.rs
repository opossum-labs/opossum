use num::Zero;
use opossum_core::{
    OpmDocument,
    analyzers::AnalyzerType,
    error::OpmResult,
    joule,
    lightdata::{
        energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
        light_data_builder::LightDataBuilder,
    },
    nanometer,
    nodes::{BeamSplitter, EnergyMeter, NodeGroup, Source, SplittingConfigBuilder},
    optic_node::OpticNode,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("inverse beam splitter test");
    let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
        EnergyLaserLines::new(vec![(nanometer!(633.0), joule!(1.0))], nanometer!(1.0))?,
    ));
    let i_s = scenery.add_node(Source::new("Source", light_data_builder))?;
    let mut bs = BeamSplitter::new("bs", &SplittingConfigBuilder::FixedRatio(0.6)).unwrap();
    bs.set_inverted(true)?;
    let i_bs = scenery.add_node(bs)?;
    let i_d1 = scenery.add_node(EnergyMeter::new(
        "Energy meter 1",
        opossum_core::nodes::Metertype::IdealEnergyMeter,
    ))?;
    let i_d2 = scenery.add_node(EnergyMeter::new(
        "Energy meter 2",
        opossum_core::nodes::Metertype::IdealEnergyMeter,
    ))?;

    scenery.connect_nodes(i_s, "output_1", i_bs, "out1_trans1_refl2", Length::zero())?;
    scenery.connect_nodes(i_bs, "input_1", i_d1, "input_1", Length::zero())?;
    scenery.connect_nodes(i_bs, "input_2", i_d2, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/inverse_beam_splitter.opm"))
}
