use opossum::error::OpmResult;
use opossum::nodes::{
    create_point_ray_source, create_round_collimated_ray_source, Lens, ParaxialSurface,
    Propagation, RayPropagationVisualizer, WaveFront,
};
use opossum::OpticScenery;
use std::path::Path;
use uom::si::angle::degree;
use uom::si::energy::joule;
use uom::si::f64::{Angle, Energy, Length};
use uom::si::length::millimeter;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;

    // let src = scenery.add_node(create_round_collimated_ray_source(
    //     Length::new::<millimeter>(100.0),
    //     Energy::new::<joule>(1.0),
    //     10,
    // )?);
    let src = scenery.add_node(create_point_ray_source(
        Angle::new::<degree>(90.0),
        Energy::new::<joule>(1.0),
    )?);
    let s1 = scenery.add_node(Propagation::new("s1", Length::new::<millimeter>(100.0))?);
    let l1 = scenery.add_node(ParaxialSurface::new(
        "f=100",
        Length::new::<millimeter>(100.0),
    )?);
    // let s2 = scenery.add_node(Propagation::new("s2", Length::new::<millimeter>(50.0))?);
    // let l1 = scenery.add_node(Lens::new(
    //     Length::new::<millimeter>(205.55),
    //     Length::new::<millimeter>(-205.55),
    //     Length::new::<millimeter>(2.79),
    //     1.5068,
    // ));
    // let s2 = scenery.add_node(Propagation::new(
    //     "s2",
    //     Length::new::<millimeter>(404.48131),
    // )?);
    // // let s2 = scenery.add_node(Propagation::new("s2", Length::new::<millimeter>(400.0))?);
    // let l2 = scenery.add_node(Lens::new(
    //     Length::new::<millimeter>(205.55),
    //     Length::new::<millimeter>(-205.55),
    //     Length::new::<millimeter>(2.79),
    //     1.5068,
    // ));
    // let s3 = scenery.add_node(Propagation::new("s1", Length::new::<millimeter>(100.0))?);
    let det = scenery.add_node(RayPropagationVisualizer::default());
    let wf = scenery.add_node(WaveFront::default());
    scenery.connect_nodes(src, "out1", s1, "front")?;
    scenery.connect_nodes(s1, "rear", l1, "front")?;
    // scenery.connect_nodes(s1, "rear", l1, "front")?;
    // scenery.connect_nodes(l1, "rear", s2, "front")?;
    // scenery.connect_nodes(s2, "rear", l2, "front")?;
    // scenery.connect_nodes(l2, "rear", s3, "front")?;
    scenery.connect_nodes(l1, "rear", wf, "in1")?;
    scenery.connect_nodes(wf, "out1", det, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/lens_system.opm"))?;
    Ok(())
}
