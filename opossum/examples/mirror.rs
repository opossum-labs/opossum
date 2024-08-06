use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, RayPropagationVisualizer, SpotDiagram, ThinMirror, WaveFront,
    },
    optical::Alignable,
    OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    let i_src = scenery.add_node(round_collimated_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        3,
    )?);
    let i_m1 = scenery.add_node(
        ThinMirror::new("mirror 1 mirrorrrobsyjhsdfskmdhfsdf")
            .with_tilt(degree!(22.5, 0.0, 0.0))?,
    );
    let i_m2 = scenery.add_node(
        ThinMirror::new("mirror 2")
            .with_curvature(millimeter!(-100.0))?
            .with_tilt(degree!(22.5, 0.0, 0.0))?,
    );
    let i_prop_vis = scenery.add_node(RayPropagationVisualizer::default());
    let i_sd = scenery.add_node(SpotDiagram::default());
    let i_wf = scenery.add_node(WaveFront::default());

    scenery.connect_nodes(i_src, "out1", i_m1, "input", millimeter!(100.0))?;
    scenery.connect_nodes(i_m1, "reflected", i_m2, "input", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "reflected", i_prop_vis, "in1", millimeter!(80.0))?;
    scenery.connect_nodes(i_prop_vis, "out1", i_sd, "in1", millimeter!(0.1))?;
    scenery.connect_nodes(i_sd, "out1", i_wf, "in1", millimeter!(0.1))?;

    scenery.save_to_file(Path::new("./opossum/playground/mirror.opm"))?;
    Ok(())
}
