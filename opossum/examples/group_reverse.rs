use std::path::Path;

use num::Zero;
use opossum::{
    error::OpmResult,
    lightdata::{DataEnergy, LightData},
    nodes::{Dummy, EnergyMeter, NodeGroup, Source},
    optical::Optical,
    spectrum_helper::create_he_ne_spec,
    OpticScenery,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Inverse Group test".into())?;

    let i_s = scenery.add_node(Source::new(
        "Source",
        &LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0)?,
        }),
    ));

    let mut group = NodeGroup::default();
    group.expand_view(true).unwrap();
    let g_n1 = group.add_node(Dummy::new("node1"))?;
    let g_n2 = group.add_node(Dummy::new("node2"))?;

    group.connect_nodes(g_n1, "rear", g_n2, "front", Length::zero())?;
    group.map_input_port(g_n1, "front", "in1")?;
    group.map_output_port(g_n2, "rear", "out1")?;
    group.set_property("inverted", true.into()).unwrap();

    let i_g = scenery.add_node(group);
    let i_d = scenery.add_node(EnergyMeter::default());

    scenery.connect_nodes(i_s, "out1", i_g, "out1", Length::zero())?;
    scenery.connect_nodes(i_g, "in1", i_d, "in1", Length::zero())?;

    scenery.save_to_file(Path::new("./opossum/playground/group_reverse.opm"))?;
    Ok(())
}
