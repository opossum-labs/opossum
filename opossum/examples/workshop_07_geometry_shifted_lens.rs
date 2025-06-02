use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    error::OpmResult,
    joule, millimeter, nanometer,
    nodes::{Lens, NodeGroup, RayPropagationVisualizer, WaveFront, collimated_line_ray_source},
    optic_node::{Alignable, OpticNode},
    optic_ports::PortType,
    refractive_index::refr_index_schott::RefrIndexSchott,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Geometry, shifted lens");
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(40.0),
        joule!(1.0),
        9,
    )?)?;
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let mut lens1 = Lens::new(
        "75 mm lens (y shifted)",
        millimeter!(122.25),
        millimeter!(-122.25),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?
    .with_decenter(millimeter!(0.0, 5.0, 0.0))?;
    let circle = CircleConfig::new(millimeter!(25.), millimeter!(0., 0.))?;
    lens1.set_aperture(&PortType::Input, "input_1", &Aperture::BinaryCircle(circle))?;
    let i_pl1 = scenery.add_node(lens1)?;
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let i_pl2 = scenery.add_node(lens2)?;
    let mut ray_prop_vis = RayPropagationVisualizer::new("after telecope", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // let i_sd4 = scenery.add_node(WaveFront::new("wavefront after telecope"))?;
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;
    // scenery.connect_nodes(i_sd3, "output_1", i_sd4, "input_1", millimeter!(10.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/workshop_07_geometry_shifted_lens.opm",
    ))
}
