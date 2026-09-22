//! Kepler Telescope with Spherical Lenses Example
//!
//! This example builds a Kepler telescope using spherical lenses and a
//! wavelength-dependent refractive index model, then runs a ray-tracing
//! analysis on it.
//!
//! System Overview
//! 1. One collimated line ray source
//! 2. A wavelength-dependent refractive index material ("HZF52")
//! 3. Two spherical lenses ("75 mm lens" and "50 mm lens") using that
//!    material
//! 4. A circular aperture on the first lens's input
//! 5. A ray propagation visualizer after the second lens
//! 6. A ray-tracing analyzer for the source beam
//! 7. Saving the optical system to a file for later use
//!
//! The beam is defined by:
//! - a collimated line ray distribution
//! - a beam width of 45 mm
//! - an energy of 1 joule per ray
//! - 9 rays in total
//!
//! This example demonstrates:
//! - Defining a wavelength-dependent refractive index material and using it
//!   to build spherical lenses
//! - Building an optical system from spherical lenses and an aperture
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
//! - `opossum_core::material::Material` — refractive index material type
//! - `std::{env, path::Path}` — used to find the output directory and save
//!   the `.opm` file
use opossum_core::{
    core_optics::{NodeAttrExt, OpticNodeExt},
    material::Material,
    prelude::*,
};
use std::{env, path::Path};

fn main() -> OpmResult<()> {
    // Create a container for all optical elements in the system.
    // This acts as the scene (optical bench).
    let mut scenery = NodeGroup::new("Kepler spherical lenses");
    // Add a collimated line ray source.
    // This represents incoming parallel light rays.
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;
    // Define a wavelength-dependent refractive index model.
    // This describes how the lens material behaves across wavelengths.
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let mut material_hzf52: Material = refr_index_hzf52.into();
    material_hzf52.header.name = "HZF52".to_string();
    // Create the first spherical lens with real optical parameters.
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(122.25),
        millimeter!(-122.25),
        millimeter!(10.0),
        material_hzf52.clone(),
    )?;
    // Define a circular aperture.
    let aperture = Aperture::new_circle(millimeter!(25.0), ApertureType::Hole, None)?;
    // Attach aperture to the first lens input.
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    // Add first lens to the optical scene.
    let i_pl1 = scenery.add_node(lens1)?;
    // Create the second spherical lens of the telescope.
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        material_hzf52,
    )?;
    // Add second lens to the scene.
    let i_pl2 = scenery.add_node(lens2)?;
    // Add ray propagation visualizer.
    // This only displays ray paths and does not affect optics.
    let mut ray_prop_vis = RayPropagationVisualizer::new("after telescope", None)?;
    // Set visualization properties such as ray transparency.
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    // Add visualizer to the scene.
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // Connect source → first lens.
    // Distance: 20 mm.
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    // Connect first lens → second lens.
    // Distance: 125 mm.
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    // Connect second lens → visualizer.
    // Distance: 50 mm.
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;
    // Wrap the optical system into a document.
    // This stores the full setup for simulation or export.
    let mut doc = OpmDocument::new(scenery);
    // Create default ray tracing configuration.
    let mut config = RayTraceConfig::default();
    // Define ray generation from the source:
    // - beam width: 45 mm
    // - energy per ray: 1 joule
    // - number of rays: 9
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(45.0), joule!(1.0), 9)?,
    );
    // Attach ray tracing analyzer.
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Read the output directory from the environment, fallback to playground
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_01_kepler_real_lenses.opm");

    // Save the complete optical system to a file.
    // This file can be reopened in the framework for visualization or analysis.
    doc.save_to_file(&out_path)
}
