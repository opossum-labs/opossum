use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    error::OpmResult,
    joule, meter,
    nodes::{
        round_collimated_ray_source, FluenceDetector, NodeGroup, ParaxialSurface,
        RayPropagationVisualizer, Spectrometer, SpectrometerType, SpotDiagram, WaveFront,
    },
    OpmDocument,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Wavefont Demo");
    let source = round_collimated_ray_source(meter!(5e-3), joule!(1.), 15)?;
    let i_s = scenery.add_node(source)?;
    let i_wf1 = scenery.add_node(WaveFront::new("wf_monitor 1"))?;
    let i_l = scenery.add_node(ParaxialSurface::new("lens", meter!(0.1))?)?;
    let i_wf2 = scenery.add_node(WaveFront::new("wf_monitor 2"))?;
    let i_sp = scenery.add_node(SpotDiagram::new("spot 3"))?;
    let i_l2 = scenery.add_node(ParaxialSurface::new("lens", meter!(0.1))?)?;
    let i_wf3 = scenery.add_node(WaveFront::new("wf_mon3"))?;
    let i_r1 = scenery.add_node(RayPropagationVisualizer::new("ray_mon1", None)?)?;
    let i_s1 = scenery.add_node(Spectrometer::new("spec_mon", SpectrometerType::Ideal))?;
    let i_fl1 = scenery.add_node(FluenceDetector::new("fluence monitor"))?;

    scenery.connect_nodes(i_s, "output_1", i_wf1, "input_1", meter!(0.1))?;
    scenery.connect_nodes(i_wf1, "output_1", i_l, "input_1", Length::zero())?;
    scenery.connect_nodes(i_l, "output_1", i_wf2, "input_1", meter!(0.2))?;
    scenery.connect_nodes(i_wf2, "output_1", i_sp, "input_1", Length::zero())?;
    scenery.connect_nodes(i_sp, "output_1", i_l2, "input_1", Length::zero())?;
    scenery.connect_nodes(i_l2, "output_1", i_wf3, "input_1", Length::zero())?;
    scenery.connect_nodes(i_wf3, "output_1", i_r1, "input_1", Length::zero())?;
    scenery.connect_nodes(i_r1, "output_1", i_s1, "input_1", Length::zero())?;
    scenery.connect_nodes(i_s1, "output_1", i_fl1, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/wavefront.opm"))
}
