use num::Zero;
use opossum::{
    error::OpmResult, joule, meter, nodes::{
        round_collimated_ray_source, FluenceDetector, ParaxialSurface, RayPropagationVisualizer,
        Spectrometer, SpectrometerType, SpotDiagram, WaveFront,
    }, OpmDocument, OpticScenery
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("Wavefont Demo");
    let source = round_collimated_ray_source(meter!(5e-3), joule!(1.), 15)?;
    let i_s = scenery.add_node(source);
    let i_wf1 = scenery.add_node(WaveFront::new("wf_monitor 1"));
    let i_l = scenery.add_node(ParaxialSurface::new("lens", meter!(0.1))?);
    let i_wf2 = scenery.add_node(WaveFront::new("wf_monitor 2"));
    let i_sp = scenery.add_node(SpotDiagram::new("spot 3"));
    let i_l2 = scenery.add_node(ParaxialSurface::new("lens", meter!(0.1))?);
    let i_wf3 = scenery.add_node(WaveFront::new("wf_mon3"));
    let i_r1 = scenery.add_node(RayPropagationVisualizer::new("ray_mon1", None)?);
    let i_s1 = scenery.add_node(Spectrometer::new("spec_mon", SpectrometerType::Ideal));
    let i_fl1 = scenery.add_node(FluenceDetector::new("fluence monitor"));

    scenery.connect_nodes(i_s, "out1", i_wf1, "in1", meter!(0.1))?;
    scenery.connect_nodes(i_wf1, "out1", i_l, "front", Length::zero())?;
    scenery.connect_nodes(i_l, "rear", i_wf2, "in1", meter!(0.2))?;
    scenery.connect_nodes(i_wf2, "out1", i_sp, "in1", Length::zero())?;
    scenery.connect_nodes(i_sp, "out1", i_l2, "front", Length::zero())?;
    scenery.connect_nodes(i_l2, "rear", i_wf3, "in1", Length::zero())?;
    scenery.connect_nodes(i_wf3, "out1", i_r1, "in1", Length::zero())?;
    scenery.connect_nodes(i_r1, "out1", i_s1, "in1", Length::zero())?;
    scenery.connect_nodes(i_s1, "out1", i_fl1, "in1", Length::zero())?;

    OpmDocument::new(scenery).save_to_file(Path::new("./opossum/playground/wavefront.opm"))
}
