use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, Lens, NodeGroup, RayPropagationVisualizer, SpotDiagram,
        WaveFront,
    },
    optic_node::OpticNode,
    optic_ports::PortType,
    refractive_index::RefrIndexConst,
    OpmDocument,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let src = scenery.add_node(&round_collimated_ray_source(
        millimeter!(5.0),
        joule!(1.0),
        5,
    )?)?;
    let l1 = scenery.add_node(&Lens::new(
        "l1",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?)?;
    let mut lens = Lens::new(
        "l1",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?;
    let circle = CircleConfig::new(millimeter!(3.0), millimeter!(0., 0.))?;
    lens.set_aperture(
        &PortType::Output,
        "output_1",
        &Aperture::BinaryCircle(circle),
    )?;
    let l2 = scenery.add_node(&lens)?;
    let det = scenery.add_node(&RayPropagationVisualizer::default())?;
    let wf = scenery.add_node(&WaveFront::default())?;
    let sd = scenery.add_node(&SpotDiagram::default())?;
    scenery.connect_nodes(&src, "output_1", &l1, "input_1", millimeter!(30.0))?;
    scenery.connect_nodes(&l1, "output_1", &l2, "input_1", millimeter!(197.22992))?;
    scenery.connect_nodes(&l2, "output_1", &wf, "input_1", millimeter!(30.0))?;
    scenery.connect_nodes(&wf, "output_1", &det, "input_1", Length::zero())?;
    scenery.connect_nodes(&det, "output_1", &sd, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/surface_wavefront.opm"))
}
