use opossum::{
    analyzers::{AnalyzerType, GhostFocusConfig},
    coatings::CoatingType,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        collimated_line_ray_source, round_collimated_ray_source, Lens, NodeGroup, SpotDiagram,
    },
    optic_node::OpticNode,
    optic_ports::PortType,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(50.0),
        joule!(1.0),
        5,
    )?)?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;

    let mut lens = Lens::default();

    // let mut lens = Wedge::new(
    //     "Wedge",
    //     millimeter!(10.0),
    //     degree!(1.0),
    //     &RefrIndexConst::new(1.5)?,
    // )?;

    // let mut lens = CylindricLens::default();

    lens.set_coating(&PortType::Input, "front", &CoatingType::Fresnel)?;
    lens.set_coating(&PortType::Output, "rear", &CoatingType::Fresnel)?;
    let i_l = scenery.add_node(lens)?;
    let i_sd2 = scenery.add_node(SpotDiagram::default())?;
    scenery.connect_nodes(i_src, "out1", i_sd, "in1", millimeter!(20.0))?;
    scenery.connect_nodes(i_sd, "out1", i_l, "front", millimeter!(80.0))?;
    scenery.connect_nodes(i_l, "rear", i_sd2, "in1", millimeter!(70.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = GhostFocusConfig::default();
    config.set_max_bounces(2);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.save_to_file(Path::new("./opossum/playground/ghost_focus.opm"))
}
