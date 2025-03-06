use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        collimated_line_ray_source, Lens, NodeGroup, NodeReference, RayPropagationVisualizer,
        ThinMirror,
    },
    optic_node::{Alignable, OpticNode},
    refractive_index::RefrIndexConst,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        6,
    )?)?;
    let i_l1 = scenery.add_node(Lens::new(
        "lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &RefrIndexConst::new(1.5)?,
    )?)?;
    let i_m2 = scenery.add_node(ThinMirror::new("mirror").with_tilt(degree!(5.0, 0.0, 0.0))?)?;
    let mut l1_ref = NodeReference::from_node(&scenery.node(i_l1)?);
    l1_ref.set_inverted(true)?;
    let i_l1_ref = scenery.add_node(l1_ref)?;
    let i_sd3 = scenery.add_node(RayPropagationVisualizer::default())?;
    scenery.connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(30.0))?;
    scenery.connect_nodes(i_l1, "output_1", i_m2, "input_1", millimeter!(90.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_l1_ref, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_l1_ref, "input_1", i_sd3, "input_1", millimeter!(50.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/lens_inverse.opm"))
}
