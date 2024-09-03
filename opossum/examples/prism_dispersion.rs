use std::path::Path;

use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{NodeGroup, RayPropagationVisualizer, Source, SpotDiagram, Wedge},
    optical::{Alignable, Optical},
    position_distributions::Grid,
    rays::Rays,
    refractive_index::RefrIndexSellmeier1,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let refr_index_hk9l = RefrIndexSellmeier1::new(
        6.14555251E-1,
        6.56775017E-1,
        1.02699346E+0,
        1.45987884E-2,
        2.87769588E-3,
        1.07653051E+2,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let beam_size_y = millimeter!(10.0);
    let nr_of_rays = 5;
    let wedge_angle_in_degree = 10.0;
    let mut rays_1w = Rays::new_uniform_collimated(
        nanometer!(1053.),
        joule!(1.),
        &Grid::new((Length::zero(), beam_size_y), (1, nr_of_rays))?,
    )?;

    let mut rays_2w = Rays::new_uniform_collimated(
        nanometer!(527.),
        joule!(1.),
        &Grid::new((Length::zero(), beam_size_y), (1, nr_of_rays))?,
    )?;

    rays_1w.add_rays(&mut rays_2w);

    let mut scenery = NodeGroup::default();
    let light = LightData::Geometric(rays_1w);
    let mut light_src = Source::new("collimated ray source", &light);
    light_src.set_isometry(Isometry::identity());
    let src = scenery.add_node(light_src)?;

    let w1 = scenery.add_node(
        Wedge::new(
            "prism 1",
            millimeter!(20.0),
            degree!(wedge_angle_in_degree),
            &refr_index_hk9l,
        )?
        .with_tilt(degree!(wedge_angle_in_degree / -2.0, 0.0, 0.0))?,
    )?;
    let wedge2 = Wedge::new(
        "prism 2",
        millimeter!(20.0),
        degree!(-1.0 * wedge_angle_in_degree),
        &refr_index_hk9l,
    )?
    .with_tilt(degree!(wedge_angle_in_degree / 2.0, 0.0, 0.0))?;
    let w2 = scenery.add_node(wedge2)?;
    let w3 = scenery.add_node(
        Wedge::new(
            "prism 3",
            millimeter!(20.0),
            degree!(-1.0 * wedge_angle_in_degree),
            &refr_index_hk9l,
        )?
        .with_tilt(degree!(wedge_angle_in_degree / 2.0, 0.0, 0.0))?,
    )?;
    let w4 = scenery.add_node(
        Wedge::new(
            "prism 4",
            millimeter!(20.0),
            degree!(wedge_angle_in_degree),
            &refr_index_hk9l,
        )?
        .with_tilt(degree!(wedge_angle_in_degree / -2.0, 0.0, 0.0))?,
    )?;
    let det = scenery.add_node(RayPropagationVisualizer::default())?;
    let sd = scenery.add_node(SpotDiagram::default())?;
    scenery.connect_nodes(src, "out1", w1, "front", millimeter!(50.0))?;
    scenery.connect_nodes(w1, "rear", w2, "front", millimeter!(100.0))?;
    scenery.connect_nodes(w2, "rear", w3, "front", millimeter!(150.0))?;
    scenery.connect_nodes(w3, "rear", w4, "front", millimeter!(100.0))?;
    scenery.connect_nodes(w4, "rear", det, "in1", millimeter!(50.0))?;
    scenery.connect_nodes(det, "out1", sd, "in1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/prism_dispersion.opm"))
}
