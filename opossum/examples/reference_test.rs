use num::Zero;
use opossum::{
    OpmDocument,
    analyzers::AnalyzerType,
    error::OpmResult,
    joule,
    lightdata::{energy_data_builder::EnergyDataBuilder, light_data_builder::LightDataBuilder},
    nanometer,
    nodes::{EnergyMeter, IdealFilter, NodeGroup, NodeReference, Source},
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Reference node demo");
    let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    ));
    let src = scenery.add_node(Source::new("source", light_data_builder))?;
    let filt = scenery.add_node(IdealFilter::new(
        "50 % filter",
        &opossum::nodes::FilterType::Constant(0.5),
    )?)?;
    let reference = scenery.add_node(NodeReference::from_node(&scenery.node(filt).unwrap()))?;
    let detector = scenery.add_node(EnergyMeter::default())?;
    scenery.connect_nodes(src, "output_1", filt, "input_1", Length::zero())?;
    scenery.connect_nodes(filt, "output_1", reference, "input_1", Length::zero())?;
    scenery.connect_nodes(reference, "output_1", detector, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/reference_test.opm"))
}
