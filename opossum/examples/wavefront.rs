use opossum::{
    error::OpmResult,
    nodes::{
        create_collimated_ray_source, create_point_ray_source, EnergyMeter, ParaxialSurface,
        Propagation, WaveFront,
    },
    OpticScenery,
};
use std::path::Path;
use uom::si::{
    angle::degree,
    energy::{self, joule},
    f64::{Angle, Energy, Length},
    length::meter,
};
fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Wavefont Demo")?;
    let source =
        create_collimated_ray_source(Length::new::<meter>(5e-3), Energy::new::<joule>(1.), 10)?;
    let i_s = scenery.add_node(source);
    let i_p1 = scenery.add_node(Propagation::new("propagation", Length::new::<meter>(0.1))?);
    let i_wf1 = scenery.add_node(WaveFront::new("wf_monitor 1"));
    let i_l = scenery.add_node(ParaxialSurface::new("lens", Length::new::<meter>(0.1))?);
    let i_p2 = scenery.add_node(Propagation::new("propagation", Length::new::<meter>(0.25))?);
    let i_wf2: petgraph::prelude::NodeIndex = scenery.add_node(WaveFront::new("wf_monitor 2"));
    let i_l2 = scenery.add_node(ParaxialSurface::new("lens", Length::new::<meter>(0.1))?);

    let i_wf3: petgraph::prelude::NodeIndex = scenery.add_node(WaveFront::default());

    scenery.connect_nodes(i_s, "out1", i_p1, "front")?;
    scenery.connect_nodes(i_p1, "rear", i_wf1, "in1")?;
    scenery.connect_nodes(i_wf1, "out1", i_l, "front")?;
    scenery.connect_nodes(i_l, "rear", i_p2, "front")?;
    scenery.connect_nodes(i_p2, "rear", i_wf2, "in1")?;
    scenery.connect_nodes(i_wf2, "out1", i_l2, "front")?;
    scenery.connect_nodes(i_l2, "rear", i_wf3, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/wavefront.opm"))?;
    Ok(())
}
