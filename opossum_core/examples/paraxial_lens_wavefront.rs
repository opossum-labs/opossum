use num_traits::Zero;
use opossum_core::{nodes::round_collimated_ray_builder, prelude::*};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Lens Ray-trace test");
    let i_src = scenery.add_node(SourcePort::new("collimated ray source"))?;

    let lens = scenery.add_node(ParaxialSurface::new("f=100 mm", millimeter!(100.0))?)?;
    let wf = scenery.add_node(WaveFront::default())?;
    let det = scenery.add_node(RayPropagationVisualizer::default())?;
    scenery.connect_nodes(i_src, "output_1", lens, "input_1", Length::zero())?;
    scenery.connect_nodes(lens, "output_1", wf, "input_1", millimeter!(90.0))?;
    scenery.connect_nodes(wf, "output_1", det, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 30)?,
    );
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/paraxial_lens_wavefront.opm",
    ))
}
