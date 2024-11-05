use opossum::{
    analyzers::{AnalyzerType, GhostFocusConfig, RayTraceConfig},
    coatings::CoatingType,
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, Lens, NodeGroup, RayPropagationVisualizer, SpotDiagram,
        ThinMirror, Wedge,
    },
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
    let i_prop = scenery.add_node(&RayPropagationVisualizer::default())?;
    let mut lens = Lens::default();
    lens.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    lens.set_coating(&PortType::Output, "output_1", &CoatingType::Fresnel)?;
    let i_l = scenery.add_node(&lens)?;

    let mut lens2 = Lens::default();
    lens2.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    lens2.set_coating(&PortType::Output, "output_1", &CoatingType::Fresnel)?;
    let i_l2 = scenery.add_node(&lens2)?;

    let mir1 = scenery.add_node(&ThinMirror::new("mir 1").with_tilt(degree!(45., 0., 0.))?)?;
    let mir2 = scenery.add_node(&ThinMirror::new("mir 2").with_tilt(degree!(-45., 0., 0.))?)?;
    let mut wedge = Wedge::new(
        "wedge",
        millimeter!(20.0),
        degree!(0.5),
        &RefrIndexConst::new(1.5)?,
    )?
    .with_tilt(degree!(0.5, 0.0, 0.0))?;
    wedge.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    wedge.set_coating(&PortType::Output, "output_1", &CoatingType::Fresnel)?;
    let i_w = scenery.add_node(&wedge)?;

    scenery.connect_nodes(i_src, "output_1", i_l, "input_1", millimeter!(120.0))?;
    // scenery.connect_nodes(i_l, "output_1", i_l2, "input_1", millimeter!(1000.0))?;
    scenery.connect_nodes(i_l, "output_1", mir1, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(mir1, "output_1", mir2, "input_1", millimeter!(700.0))?;
    scenery.connect_nodes(mir2, "output_1", i_l2, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(i_l2, "output_1", i_sd, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(i_sd, "output_1", i_prop, "input_1", millimeter!(0.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = GhostFocusConfig::default();
    config.set_max_bounces(1);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/ghost_focus.opm"))
}
