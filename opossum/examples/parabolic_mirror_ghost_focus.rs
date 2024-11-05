use opossum::{
    analyzers::{AnalyzerType, GhostFocusConfig},
    coatings::CoatingType,
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, NodeGroup, ParabolicMirror, SpotDiagram, ThinMirror, Wedge,
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
        millimeter!(20.0),
        joule!(1.0),
        3,
    )?)?;
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(45., 0.0, 0.0))?;
    mirror1.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingType::ConstantR { reflectivity: 0.5 },
    )?;
    let i_m1 = scenery.add_node(&mirror1)?;
    let i_m2 = scenery.add_node(
        &ParabolicMirror::new("parabola", millimeter!(-50.0))?
            .with_oap_angles(degree!(90.0, 0.0))?,
    )?;
    let i_m3 = scenery.add_node(
        &ParabolicMirror::new("parabola", millimeter!(-50.0))?
            .with_oap_angles(degree!(-90.0, 0.0))?,
    )?;
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
    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_m3, "input_1", millimeter!(250.0))?;
    scenery.connect_nodes(i_m3, "output_1", i_w, "input_1", millimeter!(200.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = GhostFocusConfig::default();
    config.set_max_bounces(0);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.save_to_file(Path::new(
        "./opossum/playground/parabolic_mirror_ghost_focus.opm",
    ))
}
