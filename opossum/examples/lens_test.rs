use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, Lens, RayPropagationVisualizer, Wedge},
    optical::Alignable,
    refractive_index::RefrIndexConst,
    OpmDocument, OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::default();
    scenery.set_description("Lens Ray-trace test");

    let src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        6,
    )?);
    let lens1 = Wedge::new(
        "Wedge",
        millimeter!(10.0),
        degree!(0.0),
        &RefrIndexConst::new(1.5068)?,
    )?
    .with_tilt(degree!(15.0, 0.0, 0.0))?;
    // lens1.set_property("alignment", lens1_align.into())?;
    let l1 = scenery.add_node(lens1);
    let lens2 = Lens::new(
        "Lens 2",
        millimeter!(205.55),
        millimeter!(-205.55),
        millimeter!(2.79),
        &RefrIndexConst::new(1.5068).unwrap(),
    )?
    .with_tilt(degree!(15.0, 0.0, 0.0))?;
    let l2 = scenery.add_node(lens2);
    let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot", None)?);
    //let wf = scenery.add_node(WaveFront::new("Wavefront"));
    scenery.connect_nodes(src, "out1", l1, "front", millimeter!(50.0))?;
    // scenery.connect_nodes(l1, "rear", l2, "front", millimeter!(404.44560))?;
    scenery.connect_nodes(l1, "rear", l2, "front", millimeter!(50.0))?;
    scenery.connect_nodes(l2, "rear", det, "in1", millimeter!(50.0))?;
    //scenery.connect_nodes(l2, "rear", wf, "in1", millimeter!(50.0))?;
    // scenery.connect_nodes(l2, "rear", det, "in1", millimeter!(100.0))?;

    OpmDocument::new(scenery).save_to_file(Path::new("./opossum/playground/lens_test.opm"))
}
