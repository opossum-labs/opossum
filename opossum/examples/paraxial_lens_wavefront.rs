use std::path::Path;

use opossum::{
    error::OpmResult,
    nodes::{
        create_round_collimated_ray_source, ParaxialSurface, Propagation, RayPropagationVisualizer,
        WaveFront,
    },
    OpticScenery,
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::millimeter,
};
fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;
    let src = scenery.add_node(
        create_round_collimated_ray_source(
            Length::new::<millimeter>(5.0),
            Energy::new::<joule>(1.0),
            30,
        )
        .unwrap(),
    );
    let lens = scenery.add_node(ParaxialSurface::new(
        "f=100 mm",
        Length::new::<millimeter>(100.0),
    )?);
    let dist = scenery.add_node(Propagation::new(
        "d=50 mm",
        Length::new::<millimeter>(90.0),
    )?);
    let wf = scenery.add_node(WaveFront::default());
    let det = scenery.add_node(RayPropagationVisualizer::default());
    scenery.connect_nodes(src, "out1", lens, "front")?;
    scenery.connect_nodes(lens, "rear", dist, "front")?;
    scenery.connect_nodes(dist, "rear", wf, "in1")?;
    scenery.connect_nodes(wf, "out1", det, "in1")?;
    scenery.save_to_file(Path::new(
        "./opossum/playground/paraxial_lens_wavefront.opm",
    ))?;
    Ok(())
}
