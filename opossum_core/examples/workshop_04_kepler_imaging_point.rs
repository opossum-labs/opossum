//! # Kepler Imaging Point Source Example
//!
//! This example demonstrates a Keplerian optical system for imaging a point source
//! using `opossum_core`. The system includes a point light source, two lenses,
//! a ray propagation visualizer, and spot diagram analyzers at the image plane.
//!
//! ## System Overview
//! 1. Point light source
//! 2. First lens (75 mm focal length, HZF52 glass) with circular aperture
//! 3. Second lens (50 mm focal length, HZF52 glass)
//! 4. Ray propagation visualization
//! 5. Spot diagrams at image plane
//!
//! Distances between components are specified in millimeters.
//! ## Scientific Background and Objective
//!
//! In practical optical systems, lenses are not ideal. Real lenses introduce optical aberrations
//! such as spherical aberration, chromatic aberration,etc. This experiment
//! demonstrates how a simple Keplerian imaging system can focus light from a point source and
//! visualize these effects. By tracing rays through real lenses, we can:
//!
//! - Analyze how lens design affects image quality.
//! - Understand how focusing and collimation are influenced by lens spacing and aperture size.
//! - Observe spot diagrams that reveal deviations from a perfect point focus.
//!
//! This is similar to situations with two real mirrors or lenses in telescopes or microscopes:
//! even if we try to collimate a beam, the image may show distortions due to non-ideal lens surfaces.
//! This workshop helps us understand how to minimize such aberrations and optimize imaging quality.
use opossum_core::core_optics::{NodeAttrExt, OpticNodeExt};
use opossum_core::distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines};
use opossum_core::prelude::*;
use std::env;
use std::path::Path;

fn main() -> OpmResult<()> {
    // Initialize optical system container
    let mut scenery = NodeGroup::new("Kepler imaging point src");
    // 1. Define point source
    let i_src = scenery.add_node(SourcePort::new("point source"))?;
    // 2. Define refractive index for HZF52 glass (300–2000 nm)
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    // 3. First lens: 75 mm focal length, 10 mm thickness, circular aperture of 25 mm
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(130.0),
        millimeter!(-130.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let aperture = Aperture::new_circle(millimeter!(25.0), ApertureType::Hole, None)?;
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    let i_pl1 = scenery.add_node(lens1)?;
    // 4. Second lens: 50 mm focal length, 10 mm thickness
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let i_pl2 = scenery.add_node(lens2)?;
    // 5. Ray propagation visualizer: 3D ray paths
    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // 6. Spot diagram analyzers at image plane
    let i_sd4 = scenery.add_node(SpotDiagram::new("spot at source")?)?;
    let i_sd5 = scenery.add_node(SpotDiagram::new("spot at image")?)?;
    // 7. Connect nodes in order
    // Point source → first spot diagram → lens1 → lens2 → ray propagation → second spot diagram
    scenery.connect_nodes(i_src, "output_1", i_sd4, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_sd4, "output_1", i_pl1, "input_1", millimeter!(70.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(58.0))?;
    scenery.connect_nodes(i_sd3, "output_1", i_sd5, "input_1", millimeter!(0.1))?;
    // 8. Configure ray tracing
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        RayDataSource::PointSrc(PointSrc::new(
            Hexapolar::new(millimeter!(15.0), 8)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
            millimeter!(70.0),
        )?)
        .into(),
    );
    // 9. Add ray trace analyzer and save
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Read the output directory from the environment, fallback to playground
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_04_kepler_imaging_point.opm");

    // Save the complete optical system to a file.
    // This file can be reopened in the framework for visualization or analysis.
    doc.save_to_file(&out_path)
}
