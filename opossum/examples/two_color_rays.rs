use std::path::Path;

use opossum::{error::OpmResult, lightdata::LightData, nodes::{Lens, Propagation, RayPropagationVisualizer, Source, SpotDiagram, WaveFront}, position_distributions::Hexapolar, rays::Rays, refractive_index::RefrIndexConst, OpticScenery};
use uom::si::{energy::joule, f64::{Energy, Length}, length::{millimeter, nanometer}};

fn main() -> OpmResult<()>{


    let mut rays_1w = Rays::new_uniform_collimated(
        Length::new::<nanometer>(1053.),
        Energy::new::<joule>(1.),
        &Hexapolar::new(
            Length::new::<millimeter>(2.),
            1,
        )?,
    )?;

    let mut rays_2w = Rays::new_uniform_collimated(
        Length::new::<nanometer>(527.),
        Energy::new::<joule>(1.),
        &Hexapolar::new(
            Length::new::<millimeter>(1.3),
            1,
        )?,
    )?;

    rays_1w.add_rays(&mut rays_2w);

    let mut scenery = OpticScenery::new();
    let light = LightData::Geometric(rays_1w);
    let src = scenery.add_node(Source::new("collimated ray source", &light));
    let s1 = scenery.add_node(Propagation::new("s1", Length::new::<millimeter>(30.0))?);
    let l1 = scenery.add_node(Lens::new(
        "l1",
        Length::new::<millimeter>(200.0),
        Length::new::<millimeter>(-200.0),
        Length::new::<millimeter>(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?);
    let s2 = scenery.add_node(Propagation::new(
        "s2",
        Length::new::<millimeter>(197.22992),
    )?);
    let l2 = scenery.add_node(Lens::new(
        "l1",
        Length::new::<millimeter>(200.0),
        Length::new::<millimeter>(-200.0),
        Length::new::<millimeter>(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?);
    let s3 = scenery.add_node(Propagation::new("s3", Length::new::<millimeter>(30.0))?);
    let det = scenery.add_node(RayPropagationVisualizer::default());
    // let wf = scenery.add_node(WaveFront::default());
    let sd = scenery.add_node(SpotDiagram::default());
    scenery.connect_nodes(src, "out1", s1, "front")?;
    scenery.connect_nodes(s1, "rear", l1, "front")?;
    scenery.connect_nodes(l1, "rear", s2, "front")?;
    scenery.connect_nodes(s2, "rear", l2, "front")?;
    scenery.connect_nodes(l2, "rear", s3, "front")?;
    scenery.connect_nodes(s3, "rear", det, "in1")?;
    // scenery.connect_nodes(sd, "out1", det, "in1")?;
    scenery.connect_nodes(det, "out1", sd, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/two_color_spot_diagram.opm"))?;
    Ok(())
}