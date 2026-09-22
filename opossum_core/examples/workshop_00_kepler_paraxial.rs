//! Kepler Paraxial Telescope Example
//!
//! This example builds a simple paraxial optical system and runs a
//! ray-tracing analysis on it, using a collimated line source.
//!
//! System Overview
//! 1. One collimated line ray source
//! 2. Two paraxial lenses ("75 mm lens" and "50 mm lens")
//! 3. A circular aperture on the first lens's input
//! 4. A ray propagation visualizer after the second lens
//! 5. A ray-tracing analyzer for the source beam
//! 6. Saving the optical system to a file for later use
//!
//! The beam is defined by:
//! - a collimated line ray distribution
//! - a beam width of 45 mm
//! - an energy of 1 joule per ray
//! - 9 rays in total
//!
//! This example demonstrates:
//! - Building an optical system from paraxial lenses and an aperture
//! - Connecting components with fixed propagation distances
//! - Attaching a visualizer to inspect ray paths
//! - Setting up a collimated line ray source for ray tracing
//! - Configuring a ray-tracing analyzer
//! - Saving the system as an `.opm` document
//!
//! Imports:
//! - `opossum_core::prelude::*` — optical components, analyzers, unit
//!   macros, and document types
//! - `opossum_core::core_optics::{NodeAttrExt, OpticNodeExt}` — extension
//!   traits for setting node names and properties
//! - `std::{env, path::Path}` — used to find the output directory and save
//!   the `.opm` file
use opossum_core::{
    core_optics::{NodeAttrExt, OpticNodeExt},
    prelude::*,
};
use std::{env, path::Path};
/// Entry point of the example.
///
/// This function builds the optical system step by step:
/// - defines components (source, lenses, visualizer)
/// - connects them into an optical path
/// - configures ray tracing
/// - exports the final system to a file

fn main() -> OpmResult<()> {
    // Create a container for all optical elements in the system.
    // This acts as the "scene" or optical bench.
    let mut scenery = NodeGroup::new("Kepler paraxial");
    // Add a collimated line ray source.
    // This represents incoming light rays entering the system.
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;
    // Create the first lens with a focal length of 75 mm.
    // This lens is part of the Kepler telescope setup.
    let mut lens1 = ParaxialSurface::new("75 mm lens", millimeter!(75.0))?;
    let aperture = Aperture::new_circle(millimeter!(25.0), ApertureType::Hole, None)?;
    // Attach the aperture to the first lens input.
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    // Add the first lens into the optical scene.
    let i_pl1 = scenery.add_node(lens1)?;
    // Create the second lens with a focal length of 50 mm.
    // Together with the first lens, it forms a Kepler telescope system.
    let i_pl2 = scenery.add_node(ParaxialSurface::new("50 mm lens", millimeter!(50.0))?)?;
    // Add a ray propagation visualizer.
    // This does not affect optics; it only displays ray paths.
    let mut ray_prop_vis = RayPropagationVisualizer::new("after telecope", None)?;
    // Set visualization properties such as ray transparency.
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    // Add the visualizer to the scene.
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // Connect the light source to the first lens.
    // Distance between them: 20 mm.
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    // Connect the first lens to the second lens.
    // Distance: 125 mm (typical telescope spacing region).
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    // Connect the second lens to the visualizer.
    // Distance: 50 mm.
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;
    // Wrap the entire optical scene into a document.
    // This document can be saved or analyzed later.

    let mut doc = OpmDocument::new(scenery);
    // Create a default ray tracing configuration.
    let mut config = RayTraceConfig::default();
    // Define how rays are generated from the source:
    // - beam width: 45 mm
    // - energy per ray: 1 joule
    // - number of rays: 9
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(45.0), joule!(1.0), 9)?,
    );
    // Attach ray tracing analyzer to the document.
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Read the output directory from the environment, fallback to playground
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_00_kepler_paraxial.opm");

    // Save the complete optical system to a file.
    // This file can be reopened in the framework for visualization or analysis.
    doc.save_to_file(&out_path)
}
