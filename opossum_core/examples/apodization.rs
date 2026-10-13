use opossum_core::{
    apertures::StackShape, core_optics::NodeAttrExt, geometry::body::CLEAR_APERTURE,
    nodes::round_collimated_ray_builder, prelude::*,
};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();

    let i_src = scenery.add_node(SourcePort::new("round collimated ray source"))?;

    // A dummy bounded by a square turned by 10° acts as an iris; a stack carries the rotation.
    let mut dummy = Dummy::default();
    let square = Aperture::new_rectangle(
        millimeter!(15.),
        millimeter!(15.),
        ApertureType::Hole,
        None,
        Some(degree!(10.0)),
    )?;
    dummy.set_property(
        CLEAR_APERTURE,
        ApertureShape::Stack(StackShape::new(vec![square])?).into(),
    )?;

    let i_d = scenery.add_node(dummy)?;
    let sd = SpotDiagram::default();
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
