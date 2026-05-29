use num::Zero;
use opossum_core::{analyzers::energy::EnergyConfig, prelude::*};
use std::path::Path;
use uom::si::f64::Length;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Reference node demo");
    let src = scenery.add_node(SourcePort::default())?;
    let filt = scenery.add_node(IdealFilter::new(
        "50 % filter",
        &FilterTypeBuilder::Constant(0.5),
    )?)?;
    let reference = scenery.add_node(NodeReference::from_node(&scenery.node(filt)?))?;
    let detector = scenery.add_node(EnergyMeter::default())?;
    scenery.connect_nodes(src, "output_1", filt, "input_1", Length::zero())?;
    scenery.connect_nodes(filt, "output_1", reference, "input_1", Length::zero())?;
    scenery.connect_nodes(reference, "output_1", detector, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = EnergyConfig::default();
    let energy_data_builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    )?);
    config.map_source(src, energy_data_builder);
    doc.add_analyzer(AnalyzerType::Energy(config));
    doc.save_to_file(Path::new("./opossum_core/playground/reference_test.opm"))
}
