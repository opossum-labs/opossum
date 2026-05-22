//! Geometry Shifted Lens Example
//!
//! This example demonstrates a two-lens optical system using `opossum_core`,
//! where the first lens is decentered (shifted along the y-axis).
//! Rays from a collimated source propagate through the shifted lens system
//! and are analyzed using ray visualization and wavefront detection.
//!
//! System Overview
//! 1. Collimated line ray source
//! 2. First lens shifted along the y-axis
//! 3. Second lens aligned with the optical axis
//! 4. Ray propagation visualizer
//! 5. Wavefront detector
//!
//! Distances between components are specified in millimeters.
//!
//! Principles and Objectives
//!
//! Lens decentering changes how rays propagate through an optical system.
//! This setup lets you:
//! - Observe the effect of a shifted lens on ray propagation.
//! - Understand optical misalignment in lens systems.
//! - Visualize beam deviation after passing through lenses.
//! - Analyze the resulting wavefront after propagation.
//!
//! Import `opossum_core` modules:
//! - `prelude::*` brings in core optical system types, unit macros (e.g. millimeter!, nanometer!, joule!), and optical building blocks (sources, lenses, detectors, wavefront)
use opossum_core::prelude::*;
// Import `Path` from the standard library for working with file paths
use std::path::Path;
//! Entry point for the program; returns `OpmResult<()>` to handle simulation errors safely
fn main() -> OpmResult<()> {
    // Create a NodeGroup container holding all optical components in the system
    let mut scenery = NodeGroup::new("Geometry, shifted lens");
    // Add a collimated (parallel-ray) source as the input beam
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;
    // Define the refractive index for HZF52 glass
    // over the wavelength range 300–2000 nm using the Schott model
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    // Create the first lens with:
    // - 75 mm focal geometry
    // - 10 mm thickness
    // - HZF52 refractive index
    //
    // The lens is shifted by +5 mm along the y-axis
    // using `with_decenter(...)`
    let mut lens1 = Lens::new(
        "75 mm lens (y shifted)",
        millimeter!(122.25),
        millimeter!(-122.25),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?
    .with_decenter(millimeter!(0.0, 5.0, 0.0))?;
    // Define a circular aperture with 25 mm radius
    let aperture =
        Aperture::new_circle(millimeter!(25.0), millimeter!(0., 0.), ApertureType::Hole)?;
    // Apply the aperture to the input side of the first lens
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    // Add the first lens to the optical system
    let i_pl1 = scenery.add_node(lens1)?;
    // Create the second lens aligned with the optical axis
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    // Add the second lens to the optical system
    let i_pl2 = scenery.add_node(lens2)?;
    // Create a ray propagation visualizer
    let mut ray_prop_vis = RayPropagationVisualizer::new("after telecope", None)?;
    // Set ray transparency property for clear visualization
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    // Add the ray visualizer to the system
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // Create a wavefront detector after the telescope system
    let i_sd4 = scenery.add_node(WaveFront::new("wavefront after telescope"))?;
    // Connect source → shifted lens
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    // Connect shifted lens → second lens
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    // Connect second lens → ray visualizer
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;
    // Connect ray visualizer → wavefront detector
    scenery.connect_nodes(i_sd3, "output_1", i_sd4, "input_1", millimeter!(10.0))?;
    // Create a document object for ray tracing
    let mut doc = OpmDocument::new(scenery);
     // Default configuration for ray tracing
    let mut config = RayTraceConfig::default();
    // Define a collimated ray source with specified beam width, energy, and ray sampling resolution
    // - 40 mm beam width
    // - total ray energy of 1 Joule
    // - 9 rays
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(40.0), joule!(1.0), 9)?,
    );
    // Add ray tracing analysis to the document
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    // Save the optical simulation as a reusable .opm project file
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_07_geometry_shifted_lens.opm",
    ))
}
