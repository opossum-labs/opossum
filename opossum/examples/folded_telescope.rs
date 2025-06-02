use core::f64;
use std::path::Path;

use nalgebra::Vector3;
use opossum::{
    OpmDocument,
    analyzers::{AnalyzerType, RayTraceConfig},
    centimeter,
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder},
    millimeter, nanometer,
    nodes::{Lens, NodeGroup, NodeReference, RayPropagationVisualizer, Source, ThinMirror},
    optic_node::{Alignable, OpticNode},
    position_distributions::Hexapolar,
    refractive_index::RefrIndexSellmeier1,
    spectral_distribution::Gaussian,
    utils::geom_transformation::Isometry,
};

pub fn main() -> OpmResult<()> {
    let alignment_wvl = nanometer!(1054.);
    let nbk7 = RefrIndexSellmeier1::new(
        1.039612120,
        0.231792344,
        1.010469450,
        0.00600069867,
        0.0200179144,
        103.5606530,
        nanometer!(300.)..nanometer!(1200.),
    )?;
    let mut scenery = NodeGroup::default();
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Hexapolar::new(millimeter!(10.), 10)?.into(),
        energy_dist: UniformDist::new(joule!(1.))?.into(),
        spect_dist: Gaussian::new(
            (nanometer!(1054.), nanometer!(1068.)),
            1,
            nanometer!(1054.),
            nanometer!(8.),
            1.,
        )?
        .into(),
    });
    let mut src = Source::new("collimated ray source", light_data_builder);
    src.set_alignment_wavelength(alignment_wvl)?;
    src.set_isometry(Isometry::identity())?;

    let i_src = scenery.add_node(src)?;
    // focal length = 996.7 mm (Thorlabs LA1779-B)
    let lens1 = scenery.add_node(
        Lens::new(
            "Lens 1",
            millimeter!(515.1),
            millimeter!(f64::INFINITY),
            millimeter!(3.6),
            &nbk7,
        )?
        .with_decenter(centimeter!(2., 0., 0.))?,
    )?;

    let mir_1 = ThinMirror::new("mirr").align_like_node_at_distance(lens1, millimeter!(996.7));
    let mir_1 = scenery.add_node(mir_1)?;
    let mut lens_1_ref = NodeReference::from_node(&scenery.node(lens1)?);
    lens_1_ref.set_inverted(true)?;
    let lens_1_ref = scenery.add_node(lens_1_ref)?;

    let i_prop_vis = scenery.add_node(RayPropagationVisualizer::new(
        "Ray_positions",
        Some(Vector3::y()),
    )?)?;

    scenery.connect_nodes(i_src, "output_1", lens1, "input_1", millimeter!(400.0))?;
    scenery.connect_nodes(lens1, "output_1", mir_1, "input_1", millimeter!(400.0))?;
    scenery.connect_nodes(
        mir_1,
        "output_1",
        lens_1_ref,
        "output_1",
        millimeter!(100.0),
    )?;
    scenery.connect_nodes(
        lens_1_ref,
        "input_1",
        i_prop_vis,
        "input_1",
        millimeter!(400.0),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/folded_telescope.opm"))
}
