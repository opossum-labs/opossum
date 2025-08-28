use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{NodeGroup, ParaxialSurface, RayPropagationVisualizer, collimated_line_ray_source},
    optic_node::OpticNode,
    optic_ports::PortType,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        3,
    )?)?;
    let mut lens1 = ParaxialSurface::new("100 mm lens", millimeter!(100.0))?;
    let circle = CircleConfig::new(millimeter!(25.), millimeter!(0., 0.))?;
    lens1.set_aperture(&PortType::Input, "input_1", &Aperture::BinaryCircle(circle))?;
    let i_pl1 = scenery.add_node(lens1)?;
    let i_pl2 = scenery.add_node(ParaxialSurface::new("50 mm lens", millimeter!(50.0))?)?;
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::new("after telecope", None)?)?;
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/kepler.opm"))
}
