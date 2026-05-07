//! Example: Kepler Telescope with Spherical Lenses
//!
//! This example demonstrates how to build a realistic Kepler telescope
//! using spherical lenses and a wavelength-dependent refractive index model.
//!
//! Overall structure of the system:
//! 1. Create a scene (NodeGroup)
//! 2. Add a light source
//! 3. Define refractive index material model
//! 4. Create optical elements (lenses and aperture)
//! 5. Add elements into the scene
//! 6. Connect elements using distances
//! 7. Configure ray tracing
//! 8. Run analysis and save output
use opossum_core::prelude::*;
use std::path::Path;

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
    // Create the first spherical lens with real optical parameters.
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(122.25),
        millimeter!(-122.25),
        millimeter!(10.0),
        &refr_index_hzf52,
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
        &refr_index_hzf52,
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
    // Save the optical system to file.
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_01_kepler_real_lenses.opm",
    ))
}
