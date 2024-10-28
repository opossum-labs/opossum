use opossum::{
    analyzers::{AnalyzerType, GhostFocusConfig},
    coatings::CoatingType,
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{round_collimated_ray_source, Lens, NodeGroup, SpotDiagram, Wedge},
    optic_node::{Alignable, OpticNode},
    optic_ports::PortType,
    refractive_index::RefrIndexConst,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(&round_collimated_ray_source(
        millimeter!(50.0),
        joule!(2.0),
        10,
    )?)?;
    let i_sd = scenery.add_node(&SpotDiagram::default())?;

    let mut lens = Lens::default();
    lens.set_coating(&PortType::Input, "front", &CoatingType::Fresnel)?;
    lens.set_coating(&PortType::Output, "rear", &CoatingType::Fresnel)?;
    let i_l = scenery.add_node(&lens)?;

    let mut lens2 = Lens::default();
    lens2.set_coating(&PortType::Input, "front", &CoatingType::Fresnel)?;
    lens2.set_coating(&PortType::Output, "rear", &CoatingType::Fresnel)?;

    let mut wedge = Wedge::new(
        "wedge",
        millimeter!(20.0),
        degree!(0.5),
        &RefrIndexConst::new(1.5)?,
    )?
    .with_tilt(degree!(0.5, 0.0, 0.0))?;
    wedge.set_coating(&PortType::Input, "front", &CoatingType::Fresnel)?;
    wedge.set_coating(&PortType::Output, "rear", &CoatingType::Fresnel)?;

    scenery.connect_nodes(i_src, "out1", i_l, "front", millimeter!(110.0))?;
    scenery.connect_nodes(i_l, "rear", i_sd, "in1", millimeter!(150.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = GhostFocusConfig::default();
    config.set_max_bounces(2);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.save_to_file(Path::new("./opossum/playground/ghost_focus.opm"))
}
