use opossum::{
    coatings::CoatingType, error::OpmResult, joule, millimeter, nodes::{collimated_line_ray_source, round_collimated_ray_source, EnergyMeter, FluenceDetector, Lens, RayPropagationVisualizer}, optical::Optical, refractive_index::RefrIndexConst, OpticScenery
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("Fresnel coating example".into())?;

    let src = scenery.add_node(round_collimated_ray_source(
        millimeter!(9.0),
        joule!(1.0),
        6,
    )?);
    // let src = scenery.add_node(collimated_line_ray_source(
    //     millimeter!(18.0),
    //     joule!(1.0),
    //     30,
    // )?);
    let fd1 = scenery.add_node(FluenceDetector::new("before lens"));

    let mut lens1 = Lens::new(
        "Lens",
        millimeter!(10.0),
        millimeter!(9.0),
        millimeter!(1.0),
        &RefrIndexConst::new(1.5)?,
    )?;
    lens1.set_input_coating("front", &CoatingType::Fresnel)?;
    let l1 = scenery.add_node(lens1);
    let fd2 = scenery.add_node(FluenceDetector::new("after lens"));
    let ed=scenery.add_node(EnergyMeter::default());
    let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot"));

    scenery.connect_nodes(src, "out1", fd1, "in1", millimeter!(10.0))?;
    scenery.connect_nodes(fd1, "out1", l1, "front", millimeter!(10.0))?;
    scenery.connect_nodes(l1, "rear", fd2, "in1", millimeter!(1.0))?;
    scenery.connect_nodes(fd2, "out1", ed, "in1", millimeter!(1.0))?;
    scenery.connect_nodes(ed, "out1", det, "in1", millimeter!(10.0))?;
    scenery.save_to_file(Path::new("./opossum/playground/fresnel_coating.opm"))?;
    Ok(())
}
