use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, NodeGroup, ParaxialSurface, RayPropagationVisualizer},
    optic_node::OpticNode,
    optic_ports::PortType,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Kepler paraxial");
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(45.0),
        joule!(1.0),
        9,
    )?)?;
    let mut lens1 = ParaxialSurface::new("75 mm lens", millimeter!(75.0))?;
    let circle = CircleConfig::new(millimeter!(25.), millimeter!(0., 0.))?;
    lens1.set_aperture(&PortType::Input, "input_1", &Aperture::BinaryCircle(circle))?;
    let i_pl1 = scenery.add_node(lens1)?;
    let i_pl2 = scenery.add_node(ParaxialSurface::new("50 mm lens", millimeter!(50.0))?)?;
    let mut ray_prop_vis = RayPropagationVisualizer::new("after telecope", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/workshop_00_kepler_paraxial.opm",
    ))
}
