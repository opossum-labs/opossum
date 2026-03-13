use core::f64;
use nalgebra::Vector3;
use opossum_core::lightdata::ray_data_builder::RayDataBuilder;
use opossum_core::prelude::*;
use opossum_core::{
    energy_distributions::UniformDist, position_distributions::Hexapolar,
    spectral_distribution::Gaussian,
};
use std::path::Path;

pub fn main() -> OpmResult<()> {
    let alignment_wvl = nanometer!(1054.);
    let nbk7 = RefrIndexSellmeier1::default(); // default is N-BK7
    let mut scenery = NodeGroup::default();
    let i_src = scenery.add_node(SourcePort::new("collimated ray source"))?;

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
    let mut config = RayTraceConfig::default();

    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        Hexapolar::new(millimeter!(10.), 10)?.into(),
        UniformDist::new(joule!(1.))?.into(),
        Gaussian::new(
            (nanometer!(1054.), nanometer!(1068.)),
            1,
            nanometer!(1054.),
            nanometer!(8.),
            1.,
        )?
        .into(),
    ));
    let mut ray_data_builder: RayDataBuilder = ray_data_source.into();
    ray_data_builder.set_alignment_wavelength(Some(alignment_wvl));
    config.map_source(i_src, ray_data_builder);
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/folded_telescope.opm"))
}
