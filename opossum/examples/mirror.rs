use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    coatings::CoatingType,
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, EnergyMeter, NodeGroup, RayPropagationVisualizer, SpotDiagram,
        ThinMirror, WaveFront,
    },
    optic_node::{Alignable, OpticNode},
    optic_ports::PortType,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(round_collimated_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        3,
    )?)?;
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(22.5, 0.0, 0.0))?;
    mirror1.set_coating(
        &PortType::Input,
        "input",
        &CoatingType::ConstantR { reflectivity: 0.5 },
    )?;
    let i_m1 = scenery.add_node(mirror1)?;
    let i_m2 = scenery.add_node(
        ThinMirror::new("mirror 2")
            .with_curvature(millimeter!(-100.0))?
            .with_tilt(degree!(22.5, 0.0, 0.0))?,
    )?;
    let i_prop_vis = scenery.add_node(RayPropagationVisualizer::default())?;
    let i_sd = scenery.add_node(SpotDiagram::default())?;
    let i_wf = scenery.add_node(WaveFront::default())?;
    let i_pm = scenery.add_node(EnergyMeter::default())?;
    scenery.connect_nodes(i_src, "out1", i_m1, "input", millimeter!(100.0))?;
    scenery.connect_nodes(i_m1, "reflected", i_m2, "input", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "reflected", i_prop_vis, "in1", millimeter!(80.0))?;
    scenery.connect_nodes(i_prop_vis, "out1", i_sd, "in1", millimeter!(0.1))?;
    scenery.connect_nodes(i_sd, "out1", i_wf, "in1", millimeter!(0.1))?;
    scenery.connect_nodes(i_wf, "out1", i_pm, "in1", millimeter!(0.1))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/mirror.opm"))
}
