use std::path::Path;

use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, RectangleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{Dummy, NodeGroup, SpotDiagram, round_collimated_ray_source},
    optic_node::{Alignable, OpticNode},
    optic_ports::PortType,
};

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();

    let i_src = scenery.add_node(round_collimated_ray_source(
        millimeter!(1.0),
        joule!(1.0),
        25,
    )?)?;

    let mut dummy = Dummy::default();
    let rect_config =
        RectangleConfig::new(millimeter!(15.), millimeter!(15.), millimeter!(0.0, 0.0))?;
    let aperture = Aperture::BinaryRectangle(rect_config);

    dummy.set_aperture(&PortType::Input, "input_1", &aperture)?;
    let dummy = dummy.with_decenter(millimeter!(-5.0, 5.0, 0.0))?;

    let i_d = scenery.add_node(dummy)?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;

    scenery.connect_nodes(i_src, "output_1", i_d, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_d, "output_1", i_sd, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/apodization.opm"))
}
