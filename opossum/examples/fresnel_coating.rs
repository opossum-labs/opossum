use opossum::{
    coatings::CoatingType,
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{EnergyMeter, FluenceDetector, Lens, RayPropagationVisualizer, Source},
    optical::Optical,
    position_distributions::Grid,
    rays::Rays,
    refractive_index::RefrIndexConst,
    OpmDocument, OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("Fresnel coating example".into());

    let rays = Rays::new_collimated(
        nanometer!(1000.),
        &UniformDist::new(joule!(1.))?,
        &Grid::new((millimeter!(9.), millimeter!(9.)), (100, 100))?,
    )?;
    let source = Source::new("src", &LightData::Geometric(rays));
    let src = scenery.add_node(source);
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
    let ed = scenery.add_node(EnergyMeter::default());
    let det = scenery.add_node(RayPropagationVisualizer::default());

    scenery.connect_nodes(src, "out1", fd1, "in1", millimeter!(10.0))?;
    scenery.connect_nodes(fd1, "out1", l1, "front", millimeter!(1.0))?;
    scenery.connect_nodes(l1, "rear", fd2, "in1", millimeter!(1.0))?;
    scenery.connect_nodes(fd2, "out1", ed, "in1", millimeter!(1.0))?;
    scenery.connect_nodes(ed, "out1", det, "in1", millimeter!(10.0))?;

    OpmDocument::new(scenery).save_to_file(Path::new("./opossum/playground/fresnel_coating.opm"))
}
