use std::path::Path;

use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{Lens, NodeGroup, RayPropagationVisualizer, Source, SpotDiagram},
    position_distributions::{FibonacciEllipse, Hexapolar},
    rays::Rays,
    refractive_index::RefrIndexConst,
    OpmDocument,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut rays_1w = Rays::new_uniform_collimated(
        nanometer!(1053.),
        joule!(1.),
        &FibonacciEllipse::new(millimeter!(2.), millimeter!(4.), 100)?,
    )?;

    let mut rays_2w = Rays::new_uniform_collimated(
        nanometer!(527.),
        joule!(1.),
        &Hexapolar::new(millimeter!(5.3), 4)?,
    )?;

    let mut rays_3w = Rays::new_uniform_collimated(
        nanometer!(1053. / 3.),
        joule!(1.),
        &Hexapolar::new(millimeter!(0.5), 4)?,
    )?;

    rays_1w.add_rays(&mut rays_2w);
    rays_1w.add_rays(&mut rays_3w);

    let mut scenery = NodeGroup::default();
    let light = LightData::Geometric(rays_1w);
    let src = scenery.add_node(&Source::new("collimated ray source", &light))?;
    let l1 = scenery.add_node(&Lens::new(
        "l1",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?)?;
    let l2 = scenery.add_node(&Lens::new(
        "l1",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?)?;
    let det = scenery.add_node(&RayPropagationVisualizer::default())?;
    // let wf = scenery.add_node(&WaveFront::default());
    let sd = scenery.add_node(&SpotDiagram::default())?;
    scenery.connect_nodes(&src, "output_1", &l1, "input_1", millimeter!(30.0))?;
    scenery.connect_nodes(&l1, "output_1", &l2, "input_1", millimeter!(197.22992))?;
    scenery.connect_nodes(&l2, "output_1", &det, "input_1", millimeter!(30.0))?;
    scenery.connect_nodes(&det, "output_1", &sd, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/two_color_spot_diagram.opm"))
}
