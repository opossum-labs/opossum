use std::path::Path;

use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, RectangleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{round_collimated_ray_source, Dummy, NodeGroup, SpotDiagram},
    optic_node::{Alignable, OpticNode},
    optic_ports::PortType,
    OpmDocument,
};

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();

    let i_src = scenery.add_node(&round_collimated_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        10,
    )?)?;

    let mut dummy = Dummy::default();
    let rect_config =
        RectangleConfig::new(millimeter!(15.), millimeter!(15.), millimeter!(0.0, 0.0))?;
    let aperture = Aperture::BinaryRectangle(rect_config);
    dummy.set_aperture(&PortType::Input, "front", &aperture)?;
    let dummy = dummy.with_decenter(millimeter!(-5.0, 5.0, 0.0))?;

    let i_d = scenery.add_node(&dummy)?;
    let i_sd = scenery.add_node(&SpotDiagram::default())?;

    scenery.connect_nodes(i_src, "out1", i_d, "front", millimeter!(50.0))?;
    scenery.connect_nodes(i_d, "rear", i_sd, "in1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/apodization.opm"))
}
