use num::Zero;
use opossum::{
    degree,
    error::OpmResult,
    joule, meter,
    nodes::{point_ray_source, Propagation, WaveFront},
    OpticScenery,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    let source = point_ray_source(degree!(90.0), joule!(1.))?;
    let i_s = scenery.add_node(source);
    let i_p1 = scenery.add_node(Propagation::new("propagation", meter!(0.1))?);
    let i_wf1 = scenery.add_node(WaveFront::new("wf_monitor 1"));

    scenery.connect_nodes(i_s, "out1", i_p1, "front", Length::zero())?;
    scenery.connect_nodes(i_p1, "rear", i_wf1, "in1", Length::zero())?;
    scenery.save_to_file(Path::new("./opossum/playground/point_src_wavefront.opm"))?;
    Ok(())
}
