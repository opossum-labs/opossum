use opossum_core::distributions::spectral::LaserLines;
use opossum_core::prelude::*;
use opossum_core::{
    core_optics::hit_map::fluence_estimator::FluenceEstimator,
    distributions::energy::General2DGaussian, distributions::position::SobolDist,
};
use std::{f64::consts::PI, path::Path};
use uom::si::{length::millimeter, radiant_exposure::millijoule_per_square_centimeter};
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::default())?;
    let mut fd = FluenceDetector::new("Binning");
    fd.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_fl1 = scenery.add_node(fd)?;

    let mut fd = FluenceDetector::new("Voronoi");
    fd.set_property("fluence estimator", FluenceEstimator::Voronoi.into())?;
    let i_fl2 = scenery.add_node(fd)?;

    let mut fd = FluenceDetector::new("KDE");
    fd.set_property("fluence estimator", FluenceEstimator::KDE.into())?;
    let i_fl3 = scenery.add_node(fd)?;

    scenery.connect_nodes(i_src, "output_1", i_fl1, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_fl1, "output_1", i_fl2, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_fl2, "output_1", i_fl3, "input_1", millimeter!(0.0))?;

    let mut doc = OpmDocument::new(scenery);

    let energy_dist = General2DGaussian::new(
        joule!(1.0),
        millimeter!(0.0, 0.0),
        millimeter!(10., 10.),
        1.0,
        degree!(0.0),
        false,
    )?;
    let spect_dist = LaserLines::new(vec![(nanometer!(1000.), 1.)])?;
    let pos_dist = SobolDist::new(millimeter!(100.0), millimeter!(100.0), 64000)?;

    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        pos_dist.into(),
        energy_dist.into(),
        spect_dist.into(),
    ));
    let rays = ray_data_source.clone().build()?;
    println!("# of rays {}", rays.nr_of_rays(true),);
    let focal_length = millimeter!(100.0);
    for p in vec![millimeter!(0.0)] {
        let beam_size = millimeter!(10.0) * (p - focal_length) / focal_length;
        let peak = joule!(1.0) / (2. * PI * beam_size * beam_size);
        println!(
            "theo. peak fluence @ pos {:2.} mm -> {:8.1} mJ/cm²",
            p.get::<millimeter>(),
            peak.get::<millijoule_per_square_centimeter>()
        );
    }

    let mut config = RayTraceConfig::default();
    config.map_source(i_src, ray_data_source.into());
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/fluence_test.opm"))
}
