use opossum_core::prelude::*;
use opossum_core::geometry::hit_map::fluence_estimator::FluenceEstimator;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Image field");
    let i_src = scenery.add_node(SourcePort::new("Image source"))?;
    let i_lens = scenery.add_node(
        Lens::new(
            "real lens",
            millimeter!(50.0),
            millimeter!(f64::INFINITY),
            millimeter!(10.0),
            &RefrIndexConst::new(1.5)?,
        )?
        .with_tilt(degree!(0.0, 10.0, 0.0))?,
    )?;

    let mut fluence_det = FluenceDetector::new("Camera");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_fd = scenery.add_node(fluence_det)?;

    scenery.connect_nodes(i_src, "output_1", i_lens, "input_1", millimeter!(200.0))?;
    scenery.connect_nodes(i_lens, "output_1", i_fd, "input_1", millimeter!(195.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    let ray_data_source = RayDataSource::Image(ImageSrc::new(
        Path::new("../opossum_core/logo/Logo_square_tiny.png").to_path_buf(),
        micrometer!(50.0),
        joule!(1.0),
        nanometer!(1000.0),
        degree!(2.0),
    )?);
    config.map_source(i_src, ray_data_source.into());
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/image_field.opm"))
}
