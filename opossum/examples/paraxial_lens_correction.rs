use std::path::Path;

use num::Zero;
use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        point_ray_source, ray_propagation_visualizer::RayPropagationVisualizer, ParaxialSurface,
        Propagation,
    },
    OpticScenery,
};
use petgraph::prelude::NodeIndex;
use uom::si::f64::Length;
fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;
    let src: NodeIndex = scenery.add_node(point_ray_source(degree!(1.0), joule!(1.0))?);
    let dist = scenery.add_node(Propagation::new("d=100 mm", millimeter!(100.0))?);
    let lens = scenery.add_node(ParaxialSurface::new("f=100 mm", millimeter!(100.0))?);
    scenery.connect_nodes(src, "out1", dist, "front", Length::zero())?;
    scenery.connect_nodes(dist, "rear", lens, "front", Length::zero())?;
    let dist2 = scenery.add_node(Propagation::new("gap", millimeter!(100.0))?);
    scenery.connect_nodes(lens, "rear", dist2, "front", Length::zero())?;
    let mut last_node = dist2;
    for _i in 0usize..1 {
        let l1 = scenery.add_node(ParaxialSurface::new("f", millimeter!(100.0))?);
        let d1 = scenery.add_node(Propagation::new("2f", millimeter!(200.0))?);
        let l2 = scenery.add_node(ParaxialSurface::new("f", millimeter!(100.0))?);
        let d2 = scenery.add_node(Propagation::new("gap", millimeter!(100.0))?);
        scenery.connect_nodes(last_node, "rear", l1, "front", Length::zero())?;
        scenery.connect_nodes(l1, "rear", d1, "front", Length::zero())?;
        scenery.connect_nodes(d1, "rear", l2, "front", Length::zero())?;
        scenery.connect_nodes(l2, "rear", d2, "front", Length::zero())?;
        last_node = d2;
    }
    let wf = scenery.add_node(RayPropagationVisualizer::new("ray_prop"));

    scenery.connect_nodes(last_node, "rear", wf, "in1", Length::zero())?;

    scenery.save_to_file(Path::new(
        "./opossum/playground/paraxial_lens_correction.opm",
    ))?;
    Ok(())
}
