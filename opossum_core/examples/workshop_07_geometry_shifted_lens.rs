//! Geometry Shifted Lens Example
//!
//! This example demonstrates a two-lens optical system using `opossum_core`,
//! where the first lens is decentered along the y-axis.
//! A collimated ray source propagates through the shifted lens system
//! and is analyzed using ray propagation visualization and a wavefront node.
//!
//! System Overview
//! 1. One collimated line ray source
//! 2. One decentered 75 mm lens
//! 3. One 50 mm lens
//! 4. One ray propagation visualizer
//! 5. One wavefront node after the optical system
//! 6. One ray-tracing analyzer
//!
//! The first lens is decentered by 5 mm along the y-axis.
//! Both lenses use HZF52 refractive index data over the wavelength range
//! from 300 nm to 2000 nm.
//!
//! The simulated beam is defined by:
//! - a collimated line ray distribution
//! - a beam width of 40 mm
//! - an energy of 1 joule per ray
//! - 9 rays in total
//!
//! This example demonstrates:
//! - Defining wavelength-dependent refractive index data for HZF52 glass
//! - Creating a spherical lens with a specified decenter
//! - Applying a circular aperture to a lens
//! - Connecting optical components with fixed propagation distances
//! - Configuring a ray propagation visualizer
//! - Adding a wavefront node to the optical system
//! - Configuring a ray-tracing analyzer
//! - Saving the complete optical system as an `.opm` document
//!
//! Imports:
//! - `opossum_core::prelude::*` provides optical components, analyzers,
//!   unit macros, and document types
//! - `opossum_core::core_optics::{NodeAttrExt, OpticNodeExt}` provides
//!   optical node extension traits
//! - `std::{env, path::Path}` is used to determine the output directory
//!   and save the generated `.opm` file
//! 
use opossum_core::{
    core_optics::{NodeAttrExt, OpticNodeExt},
    prelude::*,
};
/// Import `Path` from the standard library for working with file paths
use std::{env, path::Path};
/// Entry point for the program; returns `OpmResult<()>` to handle simulation errors safely
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
    // - the optical geometry defined by the specified radii of curvature
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
    // Define a circular aperture with a value of 25 mm
    let aperture = Aperture::new_circle(
        millimeter!(25.0),
        ApertureType::Hole,
        Some(millimeter!(0.0, 0.0)),
    )?;
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
    // Create a wavefront node after the optical system
    let i_sd4 = scenery.add_node(WaveFront::new("wavefront after telescope")?)?;
    // Connect source → shifted lens
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    // Connect shifted lens → second lens
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    // Connect second lens → ray visualizer
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;
    // Connect ray visualizer → wavefront node
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

    // Read the output directory from the environment, fallback to playground
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_07_geometry_shifted_lens.opm");

    // Save the complete optical system to a file.
    // This file can be reopened in the framework for visualization or analysis.
    doc.save_to_file(&out_path)
}