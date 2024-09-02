use opossum::{
    degree, error::OpmResult, joule, millimeter, nodes::{
        collimated_line_ray_source, Lens, NodeReference, RayPropagationVisualizer, ThinMirror,
    }, optical::{Alignable, Optical}, refractive_index::RefrIndexConst, OpmDocument, OpticScenery
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        6,
    )?);
    let i_l1 = scenery.add_node(Lens::new(
        "lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &RefrIndexConst::new(1.5)?,
    )?);
    let i_m2 = scenery.add_node(ThinMirror::new("mirror").with_tilt(degree!(5.0, 0.0, 0.0))?);
    let mut l1_ref = NodeReference::from_node(&scenery.node(i_l1)?);
    l1_ref.set_inverted(true)?;
    let i_l1_ref = scenery.add_node(l1_ref);
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::default());
    scenery.connect_nodes(i_src, "out1", i_l1, "front", millimeter!(30.0))?;
    scenery.connect_nodes(i_l1, "rear", i_m2, "input", millimeter!(90.0))?;
    scenery.connect_nodes(i_m2, "reflected", i_l1_ref, "rear", millimeter!(0.0))?;
    scenery.connect_nodes(i_l1_ref, "front", i_sd3, "in1", millimeter!(50.0))?;

    OpmDocument::new(scenery).save_to_file(Path::new("./opossum/playground/lens_inverse.opm"))
}
