use nalgebra::Vector3;
use opossum::degree;
use opossum::{
    centimeter,
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{Lens, NodeReference, RayPropagationVisualizer, ReflectiveGrating, Source, ThinMirror},
    num_per_mm,
    optical::{Alignable, Optical},
    position_distributions::Hexapolar,
    rays::Rays,
    refractive_index::RefrIndexSellmeier1,
    spectral_distribution::Gaussian,
    utils::geom_transformation::Isometry,
    OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    ////////////////////////////////////
    //  4 grating compressor example  //
    ////////////////////////////////////
    let alignment_wvl = nanometer!(1054.);
    let mut scenery = OpticScenery::default();
    let rays = Rays::new_collimated_with_spectrum(
        &Gaussian::new(
            (nanometer!(1040.), nanometer!(1068.)),
            50,
            nanometer!(1054.),
            nanometer!(8.),
            1.,
        )?,
        &UniformDist::new(joule!(1.))?,
        &Hexapolar::new(millimeter!(1.), 3)?,
    )?;

    let light = LightData::Geometric(rays);
    let mut src = Source::new("collimated ray source", &light);
    src.set_alignment_wavelength(alignment_wvl)?;
    src.set_isometry(Isometry::identity());

    let i_src = scenery.add_node(src);
    let i_g1 = scenery.add_node(
        ReflectiveGrating::new("grating 1", num_per_mm!(1740.), -1)?
            .with_rot_from_littrow(alignment_wvl, degree!(-4.))?,
    );

    let i_g2 = scenery.add_node(
        ReflectiveGrating::new("grating 2", num_per_mm!(1740.), -1)?
            .to_rot_from_littrow(alignment_wvl, degree!(-4.))?,
    );

    let i_g3 = scenery.add_node(
        ReflectiveGrating::new("grating 3", num_per_mm!(1740.), 1)?
            .with_rot_from_littrow(alignment_wvl, degree!(4.))?,
    );

    let i_g4 = scenery.add_node(
        ReflectiveGrating::new("grating 4", num_per_mm!(1740.), 1)?
            .to_rot_from_littrow(alignment_wvl, degree!(4.))?,
    );
    let i_prop_vis = scenery.add_node(RayPropagationVisualizer::new(
        "Ray_positions",
        Some(Vector3::y()),
    )?);

    scenery.connect_nodes(i_src, "out1", i_g1, "input", millimeter!(100.0))?;
    scenery.connect_nodes(i_g1, "diffracted", i_g2, "input", millimeter!(100.0))?;
    scenery.connect_nodes(i_g2, "diffracted", i_g3, "input", millimeter!(250.0))?;
    scenery.connect_nodes(i_g3, "diffracted", i_g4, "input", millimeter!(100.0))?;
    scenery.connect_nodes(i_g4, "diffracted", i_prop_vis, "in1", millimeter!(100.0))?;
    scenery.save_to_file(Path::new("./opossum/playground/compressor.opm"))?;

    ////////////////////////////////////
    //       Martinez Stretcher       //
    ////////////////////////////////////
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
    let mut scenery = OpticScenery::default();
    let rays = Rays::new_collimated_with_spectrum(
        &Gaussian::new(
            (nanometer!(1040.), nanometer!(1068.)),
            20,
            nanometer!(1054.),
            nanometer!(8.),
            1.,
        )?,
        &UniformDist::new(joule!(1.))?,
        &Hexapolar::new(millimeter!(1.), 0)?,
    )?;

    let light = LightData::Geometric(rays);
    let mut src = Source::new("collimated ray source", &light);
    src.set_alignment_wavelength(alignment_wvl)?;
    src.set_isometry(Isometry::identity());

    let i_src = scenery.add_node(src);
    let i_g1 = scenery.add_node(
        ReflectiveGrating::new("grating 1", num_per_mm!(1740.), -1)?
            .with_rot_from_littrow(alignment_wvl, degree!(-4.))?,
    );
    // focal length = 996.7 mm (Thorlabs LA1779-B)
    let lens1 = scenery.add_node(
        Lens::new(
            "Lens 1",
            millimeter!(515.1),
            millimeter!(f64::INFINITY),
            millimeter!(2.1),
            &nbk7,
        )?
        .with_decenter(centimeter!(0., 2., 0.))?,
    );

    let mir_1 = ThinMirror::new("mirr").align_like_node_at_distance(lens1, millimeter!(996.7));
    let mir_1 = scenery.add_node(mir_1);
    let mut lens_1_ref = NodeReference::from_node(&scenery.node(lens1)?);
    lens_1_ref.set_inverted(true)?;
    let lens_1_ref = scenery.add_node(lens_1_ref);
    let g1ref = scenery.add_node(NodeReference::from_node(&scenery.node(i_g1)?));

    let i_prop_vis = scenery.add_node(RayPropagationVisualizer::new(
        "Ray_positions",
        Some(Vector3::y()),
    )?);

    scenery.connect_nodes(i_src, "out1", i_g1, "input", millimeter!(400.0))?;
    scenery.connect_nodes(i_g1, "diffracted", lens1, "front", millimeter!(800.))?;
    scenery.connect_nodes(lens1, "rear", mir_1, "input", millimeter!(100.0))?;
    scenery.connect_nodes(mir_1, "reflected", lens_1_ref, "rear", millimeter!(100.0))?;
    scenery.connect_nodes(lens_1_ref, "front", g1ref, "input", millimeter!(100.0))?;
    scenery.connect_nodes(g1ref, "diffracted", i_prop_vis, "in1", millimeter!(1500.0))?;

    scenery.save_to_file(Path::new("./opossum/playground/martinez.opm"))?;
    Ok(())
}
