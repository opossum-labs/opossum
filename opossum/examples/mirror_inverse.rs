use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        collimated_line_ray_source, NodeGroup, NodeReference, RayPropagationVisualizer,
        SpotDiagram, ThinMirror,
    },
    optic_node::Alignable,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        10,
    )?)?;
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let i_m2 = scenery.add_node(ThinMirror::new("mirror 2").with_tilt(degree!(2.0, 0.0, 0.0))?)?;
    let m1_ref = NodeReference::from_node(&scenery.node(&i_m1)?);
    let i_m1_ref = scenery.add_node(m1_ref)?;
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::default())?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;
    let sd_ref = NodeReference::from_node(&scenery.node(&i_sd)?);
    let i_sd_ref = scenery.add_node(sd_ref)?;
    scenery.connect_nodes(&i_src, "output_1", &i_sd, "input_1", millimeter!(40.0))?;
    scenery.connect_nodes(&i_sd, "output_1", &i_m1, "input_1", millimeter!(40.0))?;
    scenery.connect_nodes(&i_m1, "output_1", &i_m2, "input_1", millimeter!(50.0))?;
    scenery.connect_nodes(&i_m2, "output_1", &i_m1_ref, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(
        &i_m1_ref,
        "output_1",
        &i_sd_ref,
        "input_1",
        millimeter!(0.0),
    )?;
    scenery.connect_nodes(&i_sd_ref, "output_1", &i_sd3, "input_1", millimeter!(20.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/mirror_inverse.opm"))
}
