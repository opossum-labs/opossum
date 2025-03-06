use std::path::Path;

use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    joule, millimeter,
    nodes::{
        round_collimated_ray_source, NodeGroup, ParaxialSurface, RayPropagationVisualizer,
        WaveFront,
    },
    OpmDocument,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Lens Ray-trace test");
    let src = scenery
        .add_node(round_collimated_ray_source(millimeter!(5.0), joule!(1.0), 30).unwrap())?;
    let lens = scenery.add_node(ParaxialSurface::new("f=100 mm", millimeter!(100.0))?)?;
    let wf = scenery.add_node(WaveFront::default())?;
    let det = scenery.add_node(RayPropagationVisualizer::default())?;
    scenery.connect_nodes(src, "output_1", lens, "input_1", Length::zero())?;
    scenery.connect_nodes(lens, "output_1", wf, "input_1", millimeter!(90.0))?;
    scenery.connect_nodes(wf, "output_1", det, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/paraxial_lens_wavefront.opm",
    ))
}
