use std::path::Path;

use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    energy_distributions::General2DGaussian,
    error::OpmResult,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{NodeGroup, Source, SpotDiagram},
    optic_node::OpticNode,
    position_distributions::HexagonalTiling,
    radian,
    rays::Rays,
    utils::geom_transformation::Isometry,
    OpmDocument,
};

fn main() -> OpmResult<()> {
    let wvl_1w = nanometer!(1054.0);
    let wvl_2w = wvl_1w / 2.0;

    let energy_1w = joule!(100.0);
    let energy_2w = joule!(50.0);

    // let beam_dist_1w = Hexapolar::new(millimeter!(76.05493), 10)?;
    let beam_dist_1w = HexagonalTiling::new(millimeter!(10.), 3, millimeter!(0., 0.))?;
    let beam_dist_2w = HexagonalTiling::new(millimeter!(10.), 3, millimeter!(1., 1.))?;
    let rays_1w = Rays::new_collimated(
        wvl_1w,
        &General2DGaussian::new(
            energy_1w,
            millimeter!(0., 0.),
            millimeter!(60.6389113608, 60.6389113608),
            5.,
            radian!(0.),
            false,
        )?,
        &beam_dist_1w,
    )?;
    let mut rays_2w = Rays::new_collimated(
        wvl_2w,
        &General2DGaussian::new(
            energy_2w,
            millimeter!(0., 0.),
            millimeter!(60.6389113608, 60.6389113608),
            5.,
            radian!(0.),
            false,
        )?,
        &beam_dist_2w,
    )?;

    let mut rays = rays_1w;
    rays.add_rays(&mut rays_2w);
    let mut scenery = NodeGroup::new("test");

    let mut src = Source::new("Source", &LightData::Geometric(rays));
    src.set_isometry(Isometry::identity())?;
    let src = scenery.add_node(&src)?;
    let i_sd = scenery.add_node(&SpotDiagram::default())?;

    scenery.connect_nodes(&src, "output_1", &i_sd, "input_1", millimeter!(100.))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    // let mut config = GhostFocusConfig::default();
    // config.set_max_bounces(0);
    // doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.save_to_file(Path::new("./opossum/playground/position_dist_test.opm"))?;
    Ok(())
}
