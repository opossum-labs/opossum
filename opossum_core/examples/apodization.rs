use opossum_core::prelude::*;
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();

    let i_src = scenery.add_node(round_collimated_ray_source(
        millimeter!(1.0),
        joule!(1.0),
        25,
    )?)?;

    let mut dummy = Dummy::default();
    let aperture = Aperture::new_rectangle(
        millimeter!(15.),
        millimeter!(15.),
        millimeter!(0.0, 0.0),
        ApertureType::Hole,
    )?;

    dummy.set_aperture(&PortType::Input, "input_1", &aperture)?;
    let dummy = dummy.with_decenter(millimeter!(-5.0, 5.0, 0.0))?;

    let i_d = scenery.add_node(dummy)?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;

    scenery.connect_nodes(i_src, "output_1", i_d, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_d, "output_1", i_sd, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum_core/playground/apodization.opm"))
}
