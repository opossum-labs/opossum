use opossum_core::prelude::*;
use opossum_core::{
    distributions::fluence::general_gaussian::General2DGaussian,
    distributions::position::Hexapolar, geometry::hit_map::fluence_estimator::FluenceEstimator,
    light::Rays, radian,
};
use std::{f64::consts::PI, path::Path};
use uom::si::radiant_exposure::joule_per_square_centimeter;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::new("Source"))?;
    let i_pl = scenery.add_node(ParaxialSurface::new("paraxial", millimeter!(500.0))?)?;
    let mut fl_det = FluenceDetector::default();
    fl_det.set_property("fluence estimator", FluenceEstimator::HelperRays.into())?;
    let i_fl1 = scenery.add_node(fl_det)?;
    let i_fl2 = scenery.add_node(FluenceDetector::default())?;

    scenery.connect_nodes(i_src, "output_1", i_fl1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_fl1, "output_1", i_pl, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_pl, "output_1", i_fl2, "input_1", millimeter!(250.))?;
    let mut doc = OpmDocument::new(scenery);

    let mut config = RayTraceConfig::default();
    let tot_energy = joule!(1.);
    let pos_dist = Hexapolar::new(millimeter!(15.), 12)?;
    let fluence_dist = General2DGaussian::new(
        tot_energy,
        millimeter!(0., 0.),
        millimeter!(4., 2.5),
        radian!(0.),
    )?;
    let rays = Rays::new_collimated_w_fluence_helper(nanometer!(1000.), &fluence_dist, &pos_dist)?;
    let peak = tot_energy / (2. * PI * millimeter!(2.5) * millimeter!(2.5));
    println!(
        "# of rays {}, theoretical peak fluence: {} J/cm²",
        rays.nr_of_rays(true),
        peak.get::<joule_per_square_centimeter>()
    );
    let ray_data_source = RayDataSource::Raw(rays);
    config.map_source(i_src, ray_data_source.into());

    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/fluence_test_w_helper.opm",
    ))
}
