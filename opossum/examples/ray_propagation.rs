use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{round_collimated_ray_source, Lens, NodeGroup, RayPropagationVisualizer},
    optic_node::OpticNode,
    optic_ports::PortType,
    refractive_index::RefrIndexConst,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    let src = scenery.add_node(&round_collimated_ray_source(
        millimeter!(5.0),
        joule!(1.0),
        10,
    )?)?;
    let l1 = scenery.add_node(&Lens::new(
        "l1",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?)?;
    let mut lens2 = Lens::new(
        "l2",
        millimeter!(200.0),
        millimeter!(-200.0),
        millimeter!(10.0),
        &RefrIndexConst::new(2.0).unwrap(),
    )?;
    lens2.set_aperture(
        &PortType::Input,
        "input_1",
        &Aperture::BinaryCircle(CircleConfig::new(millimeter!(3.), millimeter!(0., 0.))?),
    )?;
    let l2 = scenery.add_node(&lens2)?;
    let det = scenery.add_node(&RayPropagationVisualizer::default())?;
    scenery.connect_nodes(src, "output_1", l1, "input_1", millimeter!(30.0))?;
    scenery.connect_nodes(l1, "output_1", l2, "input_1", millimeter!(197.22992))?;
    scenery.connect_nodes(l2, "output_1", det, "input_1", millimeter!(30.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/ray_propagation.opm"))
}
