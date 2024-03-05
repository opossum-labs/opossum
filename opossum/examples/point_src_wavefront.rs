use opossum::{
    error::OpmResult,
    nodes::{create_point_ray_source, Propagation, WaveFront},
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
    let source = create_point_ray_source(Angle::new::<degree>(90.0), Energy::new::<joule>(1.))?;
    let i_s = scenery.add_node(source);
    let i_p1 = scenery.add_node(Propagation::new("propagation", Length::new::<meter>(0.1))?);
    let i_wf1 = scenery.add_node(WaveFront::new("wf_monitor 1"));

    scenery.connect_nodes(i_s, "out1", i_p1, "front")?;
    scenery.connect_nodes(i_p1, "rear", i_wf1, "in1")?;
    scenery.save_to_file(Path::new("./opossum/playground/point_src_wavefront.opm"))?;
    Ok(())
}
