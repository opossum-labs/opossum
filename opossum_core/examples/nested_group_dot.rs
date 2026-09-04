use num_traits::Zero;
use opossum_core::{analyzers::energy::EnergyConfig, prelude::*};
use std::path::Path;
use uom::si::f64::Length;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Nested Group Dot test");
    let i_s = scenery.add_node(SourcePort::default())?;
    let mut group = NodeGroup::default();
    let mut group2 = NodeGroup::default();
    group.set_expand_view(true)?;
    group2.set_expand_view(true)?;
    let g_n1 = group2.add_node(Dummy::new("node1"))?;
    group2.map_input_port(g_n1, "input_1", "input_1")?;
    group2.map_output_port(g_n1, "output_1", "output_1")?;
    let g_n3 = group.add_node(group2)?;

    group.map_input_port(g_n3, "input_1", "input_1")?;
    group.map_output_port(g_n3, "output_1", "output_1")?;

    let i_g = scenery.add_node(group)?;
    let i_d = scenery.add_node(EnergyMeter::default())?;

    scenery.connect_nodes(i_s, "output_1", i_g, "input_1", Length::zero())?;
    scenery.connect_nodes(i_g, "output_1", i_d, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    let energy_data_builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    )?);
    let mut config = EnergyConfig::default();
    config.map_source(i_s, energy_data_builder.into());
    doc.add_analyzer(AnalyzerType::Energy(config));
    doc.save_to_file(Path::new("./opossum_core/playground/nested_group_dot.opm"))
}
