use std::path::Path;

use opossum::{
    error::OpmResult,
    nodes::{create_point_ray_source, ParaxialSurface, Propagation, WaveFront},
    OpticScenery,
};
use petgraph::prelude::NodeIndex;
use uom::si::{
    angle::degree,
    energy::joule,
    f64::{Angle, Energy, Length},
    length::millimeter,
};
fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;
    let src: NodeIndex = scenery.add_node(create_point_ray_source(
        Angle::new::<degree>(1.0),
        Energy::new::<joule>(1.0),
    )?);
    let dist = scenery.add_node(Propagation::new(
        "d=100 mm",
        Length::new::<millimeter>(100.0),
    )?);
    let lens = scenery.add_node(ParaxialSurface::new(
        "f=100 mm",
        Length::new::<millimeter>(100.0),
    )?);
    scenery.connect_nodes(src, "out1", dist, "front")?;
    scenery.connect_nodes(dist, "rear", lens, "front")?;
    let dist2 = scenery.add_node(Propagation::new("gap", Length::new::<millimeter>(100.0))?);
    scenery.connect_nodes(lens, "rear", dist2, "front")?;
    let mut last_node = dist2;
    for _i in 0usize..1 {
        let l1 = scenery.add_node(ParaxialSurface::new("f", Length::new::<millimeter>(100.0))?);
        let d1 = scenery.add_node(Propagation::new("2f", Length::new::<millimeter>(200.0))?);
        let l2 = scenery.add_node(ParaxialSurface::new("f", Length::new::<millimeter>(100.0))?);
        let d2 = scenery.add_node(Propagation::new("gap", Length::new::<millimeter>(100.0))?);
        scenery.connect_nodes(last_node, "rear", l1, "front")?;
        scenery.connect_nodes(l1, "rear", d1, "front")?;
        scenery.connect_nodes(d1, "rear", l2, "front")?;
        scenery.connect_nodes(l2, "rear", d2, "front")?;
        last_node = d2;
    }
    let wf = scenery.add_node(WaveFront::new("wf"));

    scenery.connect_nodes(last_node, "rear", wf, "in1")?;

    scenery.save_to_file(Path::new(
        "./opossum/playground/paraxial_lens_correction.opm",
    ))?;
    Ok(())
}
