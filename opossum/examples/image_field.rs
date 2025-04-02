use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig}, degree, error::OpmResult, joule, lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder}, micrometer, millimeter, nanometer, nodes::{FluenceDetector, NodeGroup, ParaxialSurface, Source, SpotDiagram}, optic_node::OpticNode, surface::hit_map::fluence_estimator::FluenceEstimator, utils::geom_transformation::Isometry, OpmDocument
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Image field");
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Image {
        file_path: Path::new("./logo/Logo_square_tiny.png").to_path_buf(),
        pixel_size: micrometer!(50.0),
        total_energy: joule!(1.0),
        wave_length: nanometer!(1000.0),
        cone_angle: degree!(2.0)
    });
    let mut src = Source::new("image source", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    let i_src = scenery.add_node(src)?;
    let i_lens=scenery.add_node(ParaxialSurface::new("ideal lens", millimeter!(100.0))?)?;
    let mut fluence_det = FluenceDetector::new("Camera");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_fd = scenery.add_node(fluence_det)?;
    scenery.connect_nodes(i_src, "output_1", i_lens, "input_1", millimeter!(200.0))?;
    scenery.connect_nodes(i_lens, "output_1", i_fd, "input_1", millimeter!(200.0))?;
    let i_sd=scenery.add_node(SpotDiagram::default())?;
    scenery.connect_nodes(i_fd, "output_1", i_sd, "input_1", millimeter!(0.1))?;
    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/image_field.opm"))
}
