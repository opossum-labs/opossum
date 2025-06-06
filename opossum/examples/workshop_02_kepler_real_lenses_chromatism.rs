use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::{
        light_data_builder::LightDataBuilder,
        ray_data_builder::{CollimatedSrc, RayDataBuilder},
    },
    millimeter, nanometer,
    nodes::{Lens, NodeGroup, RayPropagationVisualizer, Source},
    optic_node::OpticNode,
    optic_ports::PortType,
    position_distributions::Grid,
    refractive_index::refr_index_schott::RefrIndexSchott,
    spectral_distribution::LaserLines,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Kepler chromatism");
    let light_data_builder =
        LightDataBuilder::Geometric(RayDataBuilder::Collimated(CollimatedSrc::new(
            Grid::new((Length::zero(), millimeter!(45.0)), (1, 9))?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0), (nanometer!(350.0), 1.0)])?.into(),
        )));
    let mut src = Source::new("bichromatic ray source", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    let i_src = scenery.add_node(src)?;
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
    let mut ray_prop_vis = RayPropagationVisualizer::new("after telecope", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/workshop_02_kepler_chromatism.opm",
    ))
}
