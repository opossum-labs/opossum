use std::path::Path;

use num::Zero;
use opossum::{
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, ParaxialSurface, Propagation, RayPropagationVisualizer,
        WaveFront,
    },
    OpticScenery,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;
    let src =
        scenery.add_node(round_collimated_ray_source(millimeter!(5.0), joule!(1.0), 30).unwrap());
    let lens = scenery.add_node(ParaxialSurface::new("f=100 mm", millimeter!(100.0))?);
    let dist = scenery.add_node(Propagation::new("d=50 mm", millimeter!(90.0))?);
    let wf = scenery.add_node(WaveFront::default());
    let det = scenery.add_node(RayPropagationVisualizer::default());
    scenery.connect_nodes(src, "out1", lens, "front", Length::zero())?;
    scenery.connect_nodes(lens, "rear", dist, "front", Length::zero())?;
    scenery.connect_nodes(dist, "rear", wf, "in1", Length::zero())?;
    scenery.connect_nodes(wf, "out1", det, "in1", Length::zero())?;
    scenery.save_to_file(Path::new(
        "./opossum/playground/paraxial_lens_wavefront.opm",
    ))?;
    Ok(())
}
