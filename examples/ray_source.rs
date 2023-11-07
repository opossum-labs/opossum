use std::path::Path;

use opossum::{
    error::OpmResult,
    nodes::{create_ray_source, EnergyMeter, SpotDiagram},
    OpticScenery,
};
use uom::si::{energy::joule, f64::Energy};

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Raysource demo");
    let i_s = scenery.add_node(create_ray_source(1.0, Energy::new::<joule>(1.0)));
    let i_d = scenery.add_node(EnergyMeter::default());
    let i_sd = scenery.add_node(SpotDiagram::default());
    scenery.connect_nodes(i_s, "out1", i_d, "in1")?;
    scenery.connect_nodes(i_d, "out1", i_sd, "in1")?;
    scenery.save_to_file(Path::new("playground/ray_source.opm"))?;
    Ok(())
}
