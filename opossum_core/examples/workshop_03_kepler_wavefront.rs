//! Kepler Wavefront Aberrations Example
//!
//! This example builds a Kepler telescope using real lenses and adds
//! wavefront and spot diagram analyzers to inspect the beam at different
//! points in the system.
//!
//! System Overview
//! 1. One collimated line ray source
//! 2. A wavefront analyzer before the telescope
//! 3. Refractive index data for HZF52 glass
//! 4. A 75 mm spherical lens with a circular input aperture
//! 5. A spot diagram analyzer at the focus of the first lens
//! 6. A 50 mm spherical lens
//! 7. A ray propagation visualizer after the second lens
//! 8. A wavefront analyzer after the telescope
//! 9. A ray-tracing analyzer for the source beam
//!
//! The beam is defined by:
//! - a round collimated ray distribution
//! - a beam width of 24 mm
//! - an energy of 1 joule per ray
//! - 9 rays in total
//!
//! This example demonstrates:
//! - Building spherical lenses from wavelength-dependent refractive index data
//! - Applying a circular aperture to the first lens
//! - Connecting optical components with fixed propagation distances
//! - Attaching wavefront and spot diagram analyzers to inspect the beam
//! - Configuring a ray-tracing analyzer and saving the system as an
//!   `.opm` document
//!
//! Imports:
//! - `opossum_core::prelude::*` — optical components, analyzers, unit
//!   macros, and document types
//! - `opossum_core::core_optics::{NodeAttrExt, OpticNodeExt}` — extension
//!   traits for setting node names and properties
//! - `opossum_core::nodes::round_collimated_ray_builder` — builds a round
//!   collimated ray source
//! - `std::{env, path::Path}` — used to find the output directory and save
//!   the `.opm` file

use opossum_core::{
    core_optics::{NodeAttrExt, OpticNodeExt},
    nodes::round_collimated_ray_builder,
    prelude::*,
};
use std::{env, path::Path};
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
    // 2. Define the Wavefront Analyzer
    // The wavefront analyzer captures phase information from the light
    // before it enters the telescope.
    let i_sd5 = scenery.add_node(WaveFront::new("wavefront before telescope")?)?;
    // 3. Define the Refractive Index
    // Define wavelength-dependent refractive index data for HZF52 glass
    // over the wavelength range from 300 nm to 2000 nm.
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    // 4. Define the Lenses and Aperture
    // First lens: 75 mm focal length, 10 mm thickness, with a circular aperture of 25 mm.
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(122.25),  // radius of curvature front
        millimeter!(-122.25), // radius of curvature back
        millimeter!(10.0),    // thickness
        &refr_index_hzf52,
    )?;
    let aperture = Aperture::new_circle(millimeter!(25.0), ApertureType::Hole, None)?;
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    // Add the first lens to the optical system.
    let i_pl1 = scenery.add_node(lens1)?;
    // 5. Define the Spot Diagram Analyzer
    // Add a spot diagram analyzer to inspect the beam at the focus of the first lens.
    let i_sd6 = scenery.add_node(SpotDiagram::new("spot diagram at focus")?)?;
    // Define the second lens: 50 mm focal length and 10 mm thickness.
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let i_pl2 = scenery.add_node(lens2)?;
    // 6. Define the Ray Propagation Visualizer
    // Add a ray propagation visualizer for 3D visualization of rays through the system.
    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // 7. Define the Post-Telescope Wavefront Analyzer
    // Add a wavefront analyzer to inspect the beam after it passes through the telescope.
    let i_sd4 = scenery.add_node(WaveFront::new("wavefront after telescope")?)?;
    // 8. Connect Components
    // Connect the nodes in order to create the optical path:
    // Light source → pre-telescope wavefront analyzer → first lens →
    // spot diagram analyzer → second lens → ray propagation visualizer →
    // post-telescope wavefront analyzer.
    scenery.connect_nodes(i_src, "output_1", i_sd5, "input_1", millimeter!(0.1))?;
    scenery.connect_nodes(i_sd5, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_sd6, "input_1", millimeter!(67.0))?;
    scenery.connect_nodes(i_sd6, "output_1", i_pl2, "input_1", millimeter!(58.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(20.0))?;
    scenery.connect_nodes(i_sd3, "output_1", i_sd4, "input_1", millimeter!(0.1))?;

    // 9. Configure Ray Tracing
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        round_collimated_ray_builder(millimeter!(24.0), joule!(1.0), 9)?,
    );
    // Add the configured ray-tracing analyzer to the document.
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Read the output directory from the environment, falling back to the playground directory.
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_03_kepler_wavefront.opm");

    // Save the complete optical system to an OPM file.
    // The saved file can be reopened in the framework for visualization or analysis.
    doc.save_to_file(&out_path)
}
