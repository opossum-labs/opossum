use opossum::{
    error::OpmResult,
    nodes::{create_point_ray_source, EnergyMeter, Propagation, SpotDiagram},
    OpticScenery,
};
use std::path::Path;
use uom::si::{
    angle::degree,
    energy::joule,
    f64::{Angle, Energy, Length},
    length::meter,
};

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Ray propagation demo")?;
    let source = create_point_ray_source(Angle::new::<degree>(1.0), Energy::new::<joule>(1.0))?;
    let i_s = scenery.add_node(source);
    let i_d = scenery.add_node(EnergyMeter::default());
    let i_sd = scenery.add_node(SpotDiagram::new("spot diagram 1"));
    scenery.connect_nodes(i_s, "out1", i_d, "in1")?;
    scenery.connect_nodes(i_d, "out1", i_sd, "in1")?;
    let i_p = scenery.add_node(Propagation::new("propagation", Length::new::<meter>(1.0))?);
    let i_sd2 = scenery.add_node(SpotDiagram::new("spot diagram 2"));
    scenery.connect_nodes(i_sd, "out1", i_p, "front")?;
    scenery.connect_nodes(i_p, "rear", i_sd2, "in1")?;
    scenery.save_to_file(Path::new("./opossum/playground/ray_propagation.opm"))?;
    Ok(())
}
