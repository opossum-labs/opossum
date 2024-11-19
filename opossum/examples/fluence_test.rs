use std::{f64::consts::PI, path::Path};

use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    energy_distributions::General2DGaussian,
    error::OpmResult,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{FluenceDetector, NodeGroup, Source},
    optic_node::OpticNode,
    position_distributions::{HexagonalTiling, Random, SobolDist},
    rays::Rays,
    surface::hit_map::FluenceEstimator,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use uom::si::{length::millimeter, radiant_exposure::millijoule_per_square_centimeter};
fn main() -> OpmResult<()> {
    let energy_dist = General2DGaussian::new(
        joule!(1.0),
        millimeter!(0.0, 0.0),
        millimeter!(10., 10.),
        1.0,
        degree!(0.0),
        false,
    )?;
    let pos_dist = HexagonalTiling::new(millimeter!(100.), 5)?;
    // let pos_dist=Random::new(millimeter!(100.0),millimeter!(100.0),1000)?;
    let rays = Rays::new_collimated(nanometer!(1000.), &energy_dist, &pos_dist)?;
    println!("# of rays {}", rays.nr_of_rays(true),);
    let focal_length = millimeter!(100.0);
    for p in vec![millimeter!(0.0), millimeter!(90.0)] {
        let beam_size = millimeter!(10.0) * (p - focal_length) / focal_length;
        let peak = joule!(1.0) / (2. * PI * beam_size * beam_size);
        println!(
            "theo. peak fluence @ pos {:2.} mm -> {:8.1} mJ/cm²",
            p.get::<millimeter>(),
            peak.get::<millijoule_per_square_centimeter>()
        );
    }
    let mut source = Source::new("source", &LightData::Geometric(rays));
    source.set_isometry(Isometry::identity())?;
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(&source)?;

    let mut fd = FluenceDetector::new("0 mm");
    fd.set_property("fluence estimator", FluenceEstimator::KDE.into())?;
    let i_fl1 = scenery.add_node(&fd)?;
    // let i_l = scenery.add_node(&ParaxialSurface::new("f=100mm", millimeter!(100.0))?)?;
    // let i_fl2 = scenery.add_node(&FluenceDetector::new("50 mm"))?;
    // let i_fl3 = scenery.add_node(&FluenceDetector::new("90 mm"))?;

    scenery.connect_nodes(i_src, "output_1", i_fl1, "input_1", millimeter!(5.0))?;
    // scenery.connect_nodes(i_src, "output_1", i_l, "input_1", millimeter!(10.0))?;
    // scenery.connect_nodes(i_l, "output_1", i_fl2, "input_1", millimeter!(90.0))?;
    // scenery.connect_nodes(i_fl2, "output_1", i_fl3, "input_1", millimeter!(40.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/fluence_test.opm"))
}
