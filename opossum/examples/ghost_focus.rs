use opossum::{
    analyzers::{AnalyzerType, GhostFocusConfig},
    coatings::CoatingType,
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, Lens, NodeGroup, SpotDiagram,
        Wedge,
    },
    optic_node::{Alignable, OpticNode},
    optic_ports::PortType,
    refractive_index::RefrIndexConst,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(
        // collimated_line_ray_source(millimeter!(50.), joule!(1.), 100)?
        round_collimated_ray_source(millimeter!(50.0), joule!(1.0), 7)?,
    )?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;

    let mut lens = Lens::default();

    lens.set_coating(&PortType::Input, "front", &CoatingType::Fresnel)?;
    lens.set_coating(&PortType::Output, "rear", &CoatingType::Fresnel)?;
    let i_l = scenery.add_node(lens)?;

    let mut wedge = Wedge::new(
        "wedge",
        millimeter!(20.0),
        degree!(10.0),
        &RefrIndexConst::new(1.5)?,
    )?
    .with_tilt(degree!(5.0, 0.0, 0.0))?;
    wedge.set_coating(&PortType::Input, "front", &CoatingType::Fresnel)?;
    wedge.set_coating(&PortType::Output, "rear", &CoatingType::Fresnel)?;
    let i_w = scenery.add_node(wedge)?;

    let i_sd2 = scenery.add_node(SpotDiagram::default())?;
    scenery.connect_nodes(i_src, "out1", i_sd, "in1", millimeter!(20.0))?;
    scenery.connect_nodes(i_sd, "out1", i_l, "front", millimeter!(80.0))?;
    scenery.connect_nodes(i_l, "rear", i_w, "front", millimeter!(70.0))?;
    scenery.connect_nodes(i_w, "rear", i_sd2, "in1", millimeter!(70.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = GhostFocusConfig::default();
    config.set_max_bounces(1);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    // doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/ghost_focus.opm"))
}
