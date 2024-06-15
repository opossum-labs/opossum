use opossum::{
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, Lens, RayPropagationVisualizer, Wedge},
    optical::Optical,
    refractive_index::{RefrIndexConst, RefractiveIndex},
    utils::geom_transformation::Isometry,
    OpticScenery, SceneryResources,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    //let mut global_config = SceneryResources::default();
    //global_config.ambient_refr_index = RefrIndexConst::new(1.2)?.to_enum();
    //scenery.set_global_conf(global_config);
    scenery.set_description("Lens Ray-trace test".into())?;

    let src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        6,
    )?);
    // let src = scenery.add_node(round_collimated_ray_source(
    //     millimeter!(5.0),
    //     joule!(1.0),
    //     10,
    // )?);
    // let mut lens1 = Lens::new(
    //     "Lens 1",
    //     millimeter!(205.55),
    //     millimeter!(-205.55),
    //     millimeter!(2.79),
    //     &RefrIndexConst::new(1.5068)?,
    // )?;
    // let mut lens1 = Lens::new(
    //     "Lens 1",
    //     millimeter!(f64::INFINITY),
    //     millimeter!(f64::INFINITY),
    //     millimeter!(20.0),
    //     &RefrIndexConst::new(1.5068)?,
    // )?;

    let lens1 = Wedge::new(
        "Wedge",
        millimeter!(1.0),
        degree!(0.0),
        &RefrIndexConst::new(1.5068)?,
    )?;
    // lens1.set_property("alignment", lens1_align.into())?;
    let l1 = scenery.add_node(lens1);
    let mut lens2 = Lens::new(
        "Lens 2",
        millimeter!(205.55),
        millimeter!(-205.55),
        millimeter!(2.79),
        &RefrIndexConst::new(1.5068).unwrap(),
    )?;
    let lens2_align = Some(Isometry::new(
        millimeter!(0.0, 0.0, 0.0),
        degree!(15.0, 0.0, 0.0),
    )?);
    lens2.set_property("alignment", lens2_align.into())?;
    let l2 = scenery.add_node(lens2);
    let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot"));
    //let wf = scenery.add_node(WaveFront::new("Wavefront"));
    scenery.connect_nodes(src, "out1", l1, "front", millimeter!(50.0))?;
    // scenery.connect_nodes(l1, "rear", l2, "front", millimeter!(404.44560))?;
    scenery.connect_nodes(l1, "rear", l2, "front", millimeter!(10.0))?;
    scenery.connect_nodes(l2, "rear", det, "in1", millimeter!(20.0))?;
    //scenery.connect_nodes(l2, "rear", wf, "in1", millimeter!(50.0))?;
    // scenery.connect_nodes(l2, "rear", det, "in1", millimeter!(100.0))?;

    scenery.save_to_file(Path::new("./opossum/playground/lens_test.opm"))?;
    Ok(())
}
