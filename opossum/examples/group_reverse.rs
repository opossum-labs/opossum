use std::path::Path;

use num::Zero;
use opossum::{
    OpmDocument,
    analyzers::AnalyzerType,
    error::OpmResult,
    joule,
    lightdata::{energy_data_builder::EnergyDataBuilder, light_data_builder::LightDataBuilder},
    nanometer,
    nodes::{Dummy, EnergyMeter, NodeGroup, Source},
    optic_node::OpticNode,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Inverse Group test");
    let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    ));
    let i_s = scenery.add_node(Source::new("Source", light_data_builder))?;

    let mut group = NodeGroup::default();
    group.set_expand_view(true)?;
    let g_n1 = group.add_node(Dummy::new("node1"))?;
    let g_n2 = group.add_node(Dummy::new("node2"))?;

    group.connect_nodes(g_n1, "output_1", g_n2, "input_1", Length::zero())?;
    group.map_input_port(g_n1, "input_1", "input_1")?;
    group.map_output_port(g_n2, "output_1", "output_1")?;
    group.set_inverted(true)?;

    let i_g = scenery.add_node(group)?;
    let i_d = scenery.add_node(EnergyMeter::default())?;

    scenery.connect_nodes(i_s, "output_1", i_g, "output_1", Length::zero())?;
    scenery.connect_nodes(i_g, "input_1", i_d, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::Energy);
    doc.save_to_file(Path::new("./opossum/playground/group_reverse.opm"))
}
