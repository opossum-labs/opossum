use opossum_core::{apertures::CircleShape, prelude::*};
use std::path::Path;
fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;
    // Tilted by 5°, the mirror sends the light back through the lens up to 26 mm off its axis: the
    // lens is 56 mm in diameter and still 2 mm thick at its edge.
    let i_l1 = scenery.add_node(Lens::new_with_clear_aperture(
        "lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        RefrIndexConst::new(1.5)?,
        CircleShape::new(millimeter!(28.0))?.into(),
    )?)?;
    let i_m2 = scenery.add_node(ThinMirror::new("mirror").with_tilt(degree!(5.0, 0.0, 0.0))?)?;
    let mut l1_ref = NodeReference::from_node(&scenery.node(i_l1)?)?;
    l1_ref.set_inverted(true)?;
    let i_l1_ref = scenery.add_node(l1_ref)?;
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::default())?;
    scenery.connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(30.0))?;
    scenery.connect_nodes(i_l1, "output_1", i_m2, "input_1", millimeter!(90.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_l1_ref, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_l1_ref, "input_1", i_sd3, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(20.0), joule!(1.0), 6)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/lens_inverse.opm"))
}
