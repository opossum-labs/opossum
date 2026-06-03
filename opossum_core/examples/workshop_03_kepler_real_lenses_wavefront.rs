//! # Kepler Wavefront Aberrations Example
//!
//! This example demonstrates how to set up a Keplerian optical system using `opossum_core`.
//! The system consists of a collimated light source, two real lenses (75 mm and 50 mm focal lengths),
//! and various analyzers such as wavefronts, spot diagrams, and ray propagation visualization.
//!
//! The goal is to analyze wavefront aberrations and visualize ray paths through the system.
//!
//! ## System Overview
//! 1. Collimated light source
//! 2. First lens (75 mm focal length, HZF52 glass) with an aperture
//! 3. Wavefront analysis before and after the telescope
//! 4. Spot diagram at the focus of the first lens
//! 5. Second lens (50 mm focal length, HZF52 glass)
//! 6. Ray propagation visualization
//!
//! Distances between components are specified in millimeters.
use opossum_core::{nodes::round_collimated_ray_builder, prelude::*};
use std::path::Path;
/// Entry point for the Kepler wavefront aberration example.
///
/// # Returns
/// `OpmResult<()>` – the result of building and saving the optical system.
///
/// # Description
/// This function sets up the optical components, connects them, configures ray tracing,
/// and saves the resulting `OpmDocument` to a file. It is a complete example demonstrating
/// the basic workflow in `opossum_core`.

fn main() -> OpmResult<()> {
    // Initialize the optical "scenery".
    // `NodeGroup` is a container that holds all optical nodes (sources, lenses, analyzers).
    let mut scenery = NodeGroup::new("Kepler wavefront aberrations");
    // 1. Define the Light Source
    // Create a collimated (parallel) ray source.
    // This will generate the initial rays for the optical system.
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;
    // 2. Define Wavefront Visualizers
    // WaveFront nodes capture phase information of the light at specific points in the system.
    let i_sd5 = scenery.add_node(WaveFront::new("wavefront before telescope")?)?;
    // Define refractive index data for HZF52 glass over 300–2000 nm
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    // === 4. Define Lenses and Apertures ===
    // First lens: 75 mm focal length, 10 mm thickness, circular aperture of 25 mm.
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(122.25),  // radius of curvature front
        millimeter!(-122.25), // radius of curvature back
        millimeter!(10.0),    // thickness
        &refr_index_hzf52,
    )?;
    let aperture = Aperture::new_circle(millimeter!(25.0), ApertureType::Hole, None)?;
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    // Spot diagram analyzer: visualizes the ray convergence at the focus.
    let i_pl1 = scenery.add_node(lens1)?;
    // Second lens: 50 mm focal length, 10 mm thickness..
    let i_sd6 = scenery.add_node(SpotDiagram::new("spot diagram at focus")?)?;
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let i_pl2 = scenery.add_node(lens2)?;
    // Ray propagation visualizer: for 3D visualization of rays through the system.
    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    let i_sd4 = scenery.add_node(WaveFront::new("wavefront after telescope")?)?;
    // === 5. Connect Components ===
    // Connect nodes in order to create the optical path:
    // Light source → pre-telescope wavefront → first lens → spot diagram → second lens → ray propagation → post-telescope wavefront.
    scenery.connect_nodes(i_src, "output_1", i_sd5, "input_1", millimeter!(0.1))?;
    scenery.connect_nodes(i_sd5, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_sd6, "input_1", millimeter!(67.0))?;
    scenery.connect_nodes(i_sd6, "output_1", i_pl2, "input_1", millimeter!(58.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(i_sd3, "output_1", i_sd4, "input_1", millimeter!(0.1))?;

    // === 6. Configure Ray Tracing ===
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        round_collimated_ray_builder(millimeter!(24.0), joule!(1.0), 9)?,
    );
    // === 7. Save the Document ===
    // The final optical system, including all components and analyzers, is saved as an OPM file.
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_03_kepler_wavefront.opm",
    ))
}
