use std::path::Path;

use num::Zero;
use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{point_ray_source, Dummy, ParaxialSurface, RayPropagationVisualizer},
    OpticScenery,
};
use petgraph::prelude::NodeIndex;
use uom::si::f64::Length;
fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;
    let src: NodeIndex = scenery.add_node(point_ray_source(degree!(1.0), joule!(1.0))?);
    let lens = scenery.add_node(ParaxialSurface::new("f=100 mm", millimeter!(100.0))?);

    scenery.connect_nodes(src, "out1", lens, "front", millimeter!(100.0))?;

    let dist2 = scenery.add_node(Dummy::new("gap"));
    scenery.connect_nodes(lens, "rear", dist2, "front", millimeter!(100.0))?;
    let mut last_node = dist2;
    for _i in 0usize..1 {
        let l1 = scenery.add_node(ParaxialSurface::new("f", millimeter!(100.0))?);
        let l2 = scenery.add_node(ParaxialSurface::new("f", millimeter!(100.0))?);
        scenery.connect_nodes(last_node, "rear", l1, "front", millimeter!(100.0))?;
        scenery.connect_nodes(l1, "rear", l2, "front", millimeter!(200.0))?;
        last_node = l2;
    }
    let wf = scenery.add_node(RayPropagationVisualizer::new("ray_prop"));

    scenery.connect_nodes(last_node, "rear", wf, "in1", Length::zero())?;

    scenery.save_to_file(Path::new(
        "./opossum/playground/paraxial_lens_correction.opm",
    ))?;
    Ok(())
}
