use num::Zero;
use opossum_core::{analyzers::energy::EnergyConfig, prelude::*};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Michaelson interferometer");
    let i_src = scenery.add_node(SourcePort::new("Source"))?;
    let bs = scenery.add_node(BeamSplitter::default())?;
    let sample = scenery.add_node(Dummy::new("Sample"))?;
    let rf = NodeReference::from_node(&scenery.node(sample)?)?;
    let r_sample = scenery.add_node(rf)?;
    let m1 = scenery.add_node(Dummy::new("Mirror"))?;
    let m2 = scenery.add_node(Dummy::new("Mirror"))?;
    let rf = NodeReference::from_node(&scenery.node(bs)?)?;
    let r_bs = scenery.add_node(rf)?;
    let det = scenery.add_node(Dummy::new("Detector"))?;

    scenery.connect_nodes(i_src, "output_1", bs, "input_1", Length::zero())?;
    scenery.connect_nodes(bs, "out1_trans1_refl2", sample, "input_1", Length::zero())?;
    scenery.connect_nodes(sample, "output_1", m1, "input_1", Length::zero())?;
    scenery.connect_nodes(m1, "output_1", r_sample, "input_1", Length::zero())?;
    scenery.connect_nodes(r_sample, "output_1", r_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(bs, "out2_trans2_refl1", m2, "input_1", Length::zero())?;
    scenery.connect_nodes(m2, "output_1", r_bs, "input_2", Length::zero())?;
    scenery.connect_nodes(r_bs, "out1_trans1_refl2", det, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = EnergyConfig::default();
    let energy_data_builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    )?);
    config.map_source(i_src, energy_data_builder);
    doc.add_analyzer(AnalyzerType::Energy(config));
    doc.save_to_file(Path::new("./opossum_core/playground/michaelson.opm"))
}
