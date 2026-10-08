use opossum_core::{
    apertures::CircleShape, core_optics::NodeAttrExt, geometry::body::CLEAR_APERTURE, prelude::*,
};
use std::path::Path;
fn main() -> OpmResult<()> {
    // Turned by 45°, the mirrors meet the line of ±10 mm up to 14.1 mm from their centers: they are
    // 2-inch mirrors.
    let two_inch: ApertureShape = CircleShape::new(millimeter!(25.4))?.into();
    let mut scenery = NodeGroup::default();
    let src = SourcePort::default().with_tilt(degree!(20.0, 0.0, 0.0))?;
    let i_src = scenery.add_node(src)?;
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(45.0, 0.0, 0.0))?;
    mirror1.set_property(CLEAR_APERTURE, two_inch.clone().into())?;
    let i_m1 = scenery.add_node(mirror1)?;
    let mut mirror2 = ThinMirror::new("mirror 2")
        .with_curvature(millimeter!(-100.0))?
        .with_tilt(degree!(45.0, 0.0, 0.0))?;
    mirror2.set_property(CLEAR_APERTURE, two_inch.into())?;
    let i_m2 = scenery.add_node(mirror2)?;
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::default())?;

    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_sd3, "input_1", millimeter!(100.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(20.0), joule!(1.0), 15)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/tilted_src.opm"))
}
