use opossum_core::{nodes::round_collimated_ray_builder, prelude::*};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();

    let i_src = scenery.add_node(SourcePort::new("round collimated ray source"))?;

    let mut dummy = Dummy::default();
    let aperture = Aperture::new_rectangle(
        millimeter!(15.),
        millimeter!(15.),
        millimeter!(0.0, 0.0),
        ApertureType::Hole,
        None
    )?;

    dummy.set_aperture(&PortType::Input, "input_1", &aperture)?;
    let dummy = dummy.with_decenter(millimeter!(-5.0, 5.0, 0.0))?;

    let i_d = scenery.add_node(dummy)?;
    let mut sd = SpotDiagram::default();
    sd.set_property("plot aperture", true.into())?;
    let sd_aperture = Aperture::new_rectangle(
        millimeter!(1.0),
        millimeter!(5.0),
        millimeter!(0.0, 0.0),
        ApertureType::Hole,
        None
    )?;
    sd.set_aperture(&PortType::Input, "input_1", &sd_aperture)?;
    let i_sd = scenery.add_node(sd)?;

    scenery.connect_nodes(i_src, "output_1", i_d, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_d, "output_1", i_sd, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    let ray_data_builder = round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 25)?;
    let mut config = RayTraceConfig::default();
    config.map_source(i_src, ray_data_builder);
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/apodization.opm"))
}
