use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    coatings::CoatingType,
    degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, NodeGroup, RayPropagationVisualizer, ThinMirror},
    optic_node::{Alignable, OpticNode},
    optic_ports::PortType,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Geometry, mirror system");
    let i_src = scenery.add_node(collimated_line_ray_source(
        millimeter!(20.0),
        joule!(1.0),
        9,
    )?)?;
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(22.5, 0.0, 0.0))?;
    mirror1.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingType::ConstantR { reflectivity: 0.5 },
    )?;
    let i_m1 = scenery.add_node(mirror1)?;
    let i_m2 = scenery.add_node(
        ThinMirror::new("mirror 2")
            .with_curvature(millimeter!(-100.0))?
            .with_tilt(degree!(-22.5, 0.0, 0.0))?,
    )?;
    let mut ray_prop_vis = RayPropagationVisualizer::default();
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_prop_vis = scenery.add_node(ray_prop_vis)?;
    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_prop_vis, "input_1", millimeter!(80.0))?;
    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/workshop_geometry_mirrors.opm",
    ))
}
