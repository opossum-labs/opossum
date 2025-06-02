use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::{PointSrc, RayDataBuilder}},
    millimeter, nanometer,
    nodes::{Lens, NodeGroup, NodeReference, RayPropagationVisualizer, Source, ThinMirror},
    optic_node::{Alignable, OpticNode},
    position_distributions::Grid,
    refractive_index::refr_index_schott::RefrIndexSchott,
    spectral_distribution::LaserLines,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();

    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::PointSrc (PointSrc::new(
         Grid::new((millimeter!(0.0), millimeter!(5.0)), (1, 5))?.into(),
         UniformDist::new(joule!(1.0))?.into(),
         LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
         millimeter!(75.0),
    )));
    let mut src = Source::new("point ray source", light_data_builder);
    src.set_isometry(Isometry::identity())?;

    let i_src = scenery.add_node(src)?;

    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let lens1 = Lens::new(
        "75 mm lens",
        millimeter!(122.25),
        millimeter!(-122.25),
        millimeter!(5.0),
        &refr_index_hzf52,
    )?;
    let i_l1 = scenery.add_node(lens1)?;
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(0.5, 0.0, 0.0))?)?;
    let mut l1_ref = NodeReference::from_node(&scenery.node(i_l1)?);
    l1_ref.set_inverted(true)?;
    let i_l1_ref = scenery.add_node(l1_ref)?;
    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    scenery.connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(75.5))?;
    scenery.connect_nodes(i_l1, "output_1", i_m1, "input_1", millimeter!(40.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_l1_ref, "output_1", millimeter!(40.0))?;
    scenery.connect_nodes(i_l1_ref, "input_1", i_sd3, "input_1", millimeter!(75.5))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/workshop_08_reference_node.opm",
    ))
}
