use num::Zero;
use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{Dummy, EnergyMeter, NodeGroup, SpotDiagram, round_collimated_ray_source},
    optic_node::OpticNode,
    optic_ports::PortType,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Raysource demo");
    let mut source = round_collimated_ray_source(millimeter!(1.0), joule!(1.0), 5)?;
    let aperture =
        Aperture::BinaryCircle(CircleConfig::new(millimeter!(1.0), millimeter![0.5, 0.5])?);
    source.set_aperture(&PortType::Output, "output_1", &aperture)?;
    let i_s = scenery.add_node(source)?;
    let dummy = Dummy::default();
    let i_dummy = scenery.add_node(dummy)?;
    let i_d = scenery.add_node(EnergyMeter::default())?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;
    scenery.connect_nodes(i_s, "output_1", i_dummy, "input_1", Length::zero())?;
    scenery.connect_nodes(i_dummy, "output_1", i_d, "input_1", Length::zero())?;
    scenery.connect_nodes(i_d, "output_1", i_sd, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/ray_source.opm"))
}
