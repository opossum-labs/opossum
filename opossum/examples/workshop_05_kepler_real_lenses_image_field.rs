use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    degree,
    error::OpmResult,
    joule,
    lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder},
    micrometer, millimeter, nanometer,
    nodes::{FluenceDetector, Lens, NodeGroup, Source},
    optic_node::OpticNode,
    optic_ports::PortType,
    refractive_index::refr_index_schott::RefrIndexSchott,
    surface::hit_map::fluence_estimator::FluenceEstimator,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Kepler image field");
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Image {
        file_path: Path::new("./logo/Logo_square_tiny_grey_inverted.png").to_path_buf(),
        pixel_size: micrometer!(50.0),
        total_energy: joule!(1.0),
        wave_length: nanometer!(1000.0),
        cone_angle: degree!(1.0),
    });
    let mut src = Source::new("image source", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    let i_src = scenery.add_node(src)?;
    let mut fluence_det = FluenceDetector::new("Object Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_sd5 = scenery.add_node(fluence_det)?;
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(130.0),
        millimeter!(-130.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let circle = CircleConfig::new(millimeter!(25.), millimeter!(0., 0.))?;
    lens1.set_aperture(&PortType::Input, "input_1", &Aperture::BinaryCircle(circle))?;
    let i_pl1 = scenery.add_node(lens1)?;
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let i_pl2 = scenery.add_node(lens2)?;

    let mut fluence_det = FluenceDetector::new("Before image Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_sd6 = scenery.add_node(fluence_det)?;

    let mut fluence_det = FluenceDetector::new("Image Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_sd7 = scenery.add_node(fluence_det)?;

    let mut fluence_det = FluenceDetector::new("Adter image Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_sd8 = scenery.add_node(fluence_det)?;

    scenery.connect_nodes(i_src, "output_1", i_sd5, "input_1", millimeter!(0.001))?;
    scenery.connect_nodes(i_sd5, "output_1", i_pl1, "input_1", millimeter!(70.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd6, "input_1", millimeter!(54.0))?;
    scenery.connect_nodes(i_sd6, "output_1", i_sd7, "input_1", millimeter!(4.0))?;
    scenery.connect_nodes(i_sd7, "output_1", i_sd8, "input_1", millimeter!(4.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/workshop_05_kepler_imaging_field.opm",
    ))
}
