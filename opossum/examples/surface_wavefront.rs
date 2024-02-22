use opossum::error::OpmResult;
use opossum::nodes::{
    create_point_ray_source, create_round_collimated_ray_source, Lens, Propagation, RayPropagationVisualizer, WaveFront
};
use opossum::OpticScenery;
use uom::si::angle::degree;
use std::path::Path;
use uom::si::energy::joule;
use uom::si::f64::{Angle, Energy, Length};
use uom::si::length::millimeter;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();

    let src = scenery.add_node(create_point_ray_source(
        Angle::new::<degree>(90.0),
        Energy::new::<joule>(1.0)
    )?);
    let s1 = scenery.add_node(Propagation::new("s1", Length::new::<millimeter>(100.0))?);
    let det = scenery.add_node(RayPropagationVisualizer::default());
    let wf = scenery.add_node(WaveFront::default());
    scenery.connect_nodes(src, "out1", s1, "front")?;
    scenery.connect_nodes(s1, "rear", wf, "in1")?;
    scenery.connect_nodes(wf, "out1", det, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/surface_wavefront.opm"))?;
    Ok(())
}
