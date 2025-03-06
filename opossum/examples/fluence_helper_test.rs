use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    fluence_distributions::general_gaussian::General2DGaussian,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{FluenceDetector, NodeGroup, ParaxialSurface, Source},
    optic_node::OpticNode,
    position_distributions::Hexapolar,
    radian,
    rays::Rays,
    surface::hit_map::fluence_estimator::FluenceEstimator,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use std::{f64::consts::PI, path::Path};
use uom::si::radiant_exposure::joule_per_square_centimeter;
fn main() -> OpmResult<()> {
    let tot_energy = joule!(1.);
    let pos_dist = Hexapolar::new(millimeter!(15.), 12)?;
    let fluence_dist = General2DGaussian::new(
        tot_energy,
        millimeter!(0., 0.),
        millimeter!(2.5, 2.5),
        radian!(0.),
    )?;
    let rays = Rays::new_collimated_w_fluence_helper(nanometer!(1000.), &fluence_dist, &pos_dist)?;
    let peak = tot_energy / (2. * PI * millimeter!(2.5) * millimeter!(2.5));
    println!(
        "# of rays {}, theoretical peak fluence: {} J/cm²",
        rays.nr_of_rays(true),
        peak.get::<joule_per_square_centimeter>()
    );
    let mut source = Source::new("source", &LightData::Geometric(rays));
    source.set_isometry(Isometry::identity())?;
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(source)?;
    let i_pl = scenery.add_node(ParaxialSurface::new("paraxial", millimeter!(500.0))?)?;
    let mut fl_det = FluenceDetector::default();
    fl_det.set_property("fluence estimator", FluenceEstimator::HelperRays.into())?;
    let i_fl1 = scenery.add_node(fl_det)?;
    let i_fl2 = scenery.add_node(FluenceDetector::default())?;

    scenery.connect_nodes(i_src, "output_1", i_fl1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_fl1, "output_1", i_pl, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_pl, "output_1", i_fl2, "input_1", millimeter!(250.))?;
    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/fluence_test_w_helper.opm"))
}
