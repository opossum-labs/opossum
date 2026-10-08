use opossum_core::{
    apertures::CircleShape, core_optics::NodeAttrExt, geometry::body::CLEAR_APERTURE, prelude::*,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;
    // Turned by 45°, mirror 1 meets the beam on both passes up to about 19 mm from its center:
    // it is a 2-inch mirror.
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(45.0, 0.0, 0.0))?;
    mirror1.set_property(
        CLEAR_APERTURE,
        ApertureShape::from(CircleShape::new(millimeter!(25.4))?).into(),
    )?;
    let i_m1 = scenery.add_node(mirror1)?;
    let i_m2 = scenery.add_node(ThinMirror::new("mirror 2").with_tilt(degree!(2.0, 0.0, 0.0))?)?;
    let m1_ref = NodeReference::from_node(&scenery.node(i_m1)?)?;
    let i_m1_ref = scenery.add_node(m1_ref)?;
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::default())?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;
    let sd_ref = NodeReference::from_node(&scenery.node(i_sd)?)?;
    let i_sd_ref = scenery.add_node(sd_ref)?;
    scenery.connect_nodes(i_src, "output_1", i_sd, "input_1", millimeter!(40.0))?;
    scenery.connect_nodes(i_sd, "output_1", i_m1, "input_1", millimeter!(40.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_m1_ref, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_m1_ref, "output_1", i_sd_ref, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_sd_ref, "output_1", i_sd3, "input_1", millimeter!(20.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(20.0), joule!(1.0), 10)?,
    );

    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/mirror_inverse.opm"))
}
