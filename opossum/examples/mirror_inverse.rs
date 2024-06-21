use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, NodeReference, RayPropagationVisualizer, ThinMirror},
    optical::Alignable,
    OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        6,
    )?);
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(45.0, 0.0, 0.0))?);
    let i_m2 = scenery.add_node(ThinMirror::new("mirror 2").with_tilt(degree!(2.0, 0.0, 0.0))?);
    let m1_ref = NodeReference::from_node(&scenery.node(i_m1)?);
    let i_m1_ref = scenery.add_node(m1_ref);
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::default());
    scenery.connect_nodes(i_src, "out1", i_m1, "input", millimeter!(80.0))?;
    scenery.connect_nodes(i_m1, "reflected", i_m2, "input", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "reflected", i_m1_ref, "input", millimeter!(0.0))?;
    scenery.connect_nodes(i_m1_ref, "reflected", i_sd3, "in1", millimeter!(150.0))?;

    scenery.save_to_file(Path::new("./opossum/playground/mirror_inverse.opm"))?;
    Ok(())
}
