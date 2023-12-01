use std::path::Path;

use opossum::{
    error::OpmResult,
    nodes::{create_collimated_ray_source, ParaxialSurface, Propagation, SpotDiagram},
    OpticScenery,
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::millimeter,
};

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    let i_src = scenery.add_node(create_collimated_ray_source(
        Length::new::<millimeter>(10.0),
        Energy::new::<joule>(1.0),
    )?);
    let i_sd1 = scenery.add_node(SpotDiagram::new("source"));
    let i_pl1 = scenery.add_node(ParaxialSurface::new(
        "100 mm lens",
        Length::new::<millimeter>(100.0),
    )?);
    let i_pr1 = scenery.add_node(Propagation::new(
        "100 mm",
        Length::new::<millimeter>(100.0),
    )?);
    let i_sd2 = scenery.add_node(SpotDiagram::new("focus"));
    let i_pr2 = scenery.add_node(Propagation::new("50 mm", Length::new::<millimeter>(50.0))?);
    let i_pl2 = scenery.add_node(ParaxialSurface::new(
        "50 mm lens",
        Length::new::<millimeter>(100.0),
    )?);
    let i_sd3 = scenery.add_node(SpotDiagram::new("after telecope"));
    scenery.connect_nodes(i_src, "out1", i_sd1, "in1")?;
    scenery.connect_nodes(i_sd1, "out1", i_pl1, "front")?;
    scenery.connect_nodes(i_pl1, "rear", i_pr1, "front")?;
    scenery.connect_nodes(i_pr1, "rear", i_sd2, "in1")?;
    scenery.connect_nodes(i_sd2, "out1", i_pr2, "front")?;
    scenery.connect_nodes(i_pr2, "rear", i_pl2, "front")?;
    scenery.connect_nodes(i_pl2, "rear", i_sd3, "in1")?;
    scenery.save_to_file(Path::new("./opossum/playground/kepler.opm"))?;
    Ok(())
}
