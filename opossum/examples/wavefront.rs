use opossum::{
    error::OpmResult,
    nodes::{
        create_round_collimated_ray_source, FluenceDetector, ParaxialSurface, Propagation, RayPropagationVisualizer, Spectrometer, SpectrometerType, SpotDiagram, WaveFront
    },
    OpticScenery,
};
use std::path::Path;
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::meter,
};
fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Wavefont Demo")?;
    let source = create_round_collimated_ray_source(
        Length::new::<meter>(5e-3),
        Energy::new::<joule>(1.),
        15,
    )?;
    let i_s = scenery.add_node(source);
    let i_p1 = scenery.add_node(Propagation::new("propagation", Length::new::<meter>(0.1))?);
    // let i_wf1 = scenery.add_node(WaveFront::new("wf_monitor 1"));
    let i_l = scenery.add_node(ParaxialSurface::new("lens", Length::new::<meter>(0.1))?);
    let i_p2 = scenery.add_node(Propagation::new("propagation", Length::new::<meter>(0.2))?);
    let i_wf2: petgraph::prelude::NodeIndex = scenery.add_node(WaveFront::new("wf_monitor 2"));
    let i_sp: petgraph::prelude::NodeIndex = scenery.add_node(SpotDiagram::new("spot 3"));
    let i_l2 = scenery.add_node(ParaxialSurface::new("lens", Length::new::<meter>(0.1))?);
    let i_wf3: petgraph::prelude::NodeIndex = scenery.add_node(WaveFront::new("wf_mon3"));
    let i_r1: petgraph::prelude::NodeIndex =
        scenery.add_node(RayPropagationVisualizer::new("ray_mon1"));
        let i_s1: petgraph::prelude::NodeIndex =
        scenery.add_node(Spectrometer::new("spec_mon", SpectrometerType::Ideal));
        let i_fl1: petgraph::prelude::NodeIndex =
        scenery.add_node(FluenceDetector::new("fluence monitor"));

    scenery.connect_nodes(i_s, "out1", i_p1, "front")?;
    scenery.connect_nodes(i_p1, "rear", i_l, "front")?;
    // scenery.connect_nodes(i_p1, "rear", i_wf1, "in1")?;
    // scenery.connect_nodes(i_wf1, "out1", i_l, "front")?;
    scenery.connect_nodes(i_l, "rear", i_p2, "front")?;
    scenery.connect_nodes(i_p2, "rear", i_wf2, "in1")?;
    scenery.connect_nodes(i_wf2, "out1", i_sp, "in1")?;
    scenery.connect_nodes(i_sp, "out1", i_l2, "front")?;
    scenery.connect_nodes(i_l2, "rear", i_wf3, "in1")?;
    scenery.connect_nodes(i_wf3, "out1", i_r1, "in1")?;
    scenery.connect_nodes(i_r1, "out1", i_s1, "in1")?;
    scenery.connect_nodes(i_s1, "out1", i_fl1, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/wavefront.opm"))?;
    Ok(())
}
