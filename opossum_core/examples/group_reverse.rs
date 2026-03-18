use num::Zero;
use opossum_core::{analyzers::energy::EnergyConfig, prelude::*};
use std::path::Path;
use uom::si::f64::Length;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Inverse Group test");
    let i_src = scenery.add_node(SourcePort::new("Source"))?;

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

    scenery.connect_nodes(i_src, "output_1", i_g, "output_1", Length::zero())?;
    scenery.connect_nodes(i_g, "input_1", i_d, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = EnergyConfig::default();
    let energy_data_builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![(nanometer!(633.0), joule!(1.0))],
        nanometer!(1.0),
    )?);
    config.map_source(i_src, energy_data_builder.into());
    doc.add_analyzer(AnalyzerType::Energy(config));
    doc.save_to_file(Path::new("./opossum_core/playground/group_reverse.opm"))
}
