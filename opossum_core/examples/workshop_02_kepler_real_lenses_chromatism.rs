//! Example: Kepler Telescope with Chromatic Effects
//!
//! This example demonstrates how to build a Kepler telescope using spherical lenses
//! and simulate chromatic effects using multiple wavelengths.
//!
//! Key concepts demonstrated:
//! - Wavelength-dependent refractive index (Schott model)
//! - Multi-wavelength ray generation (chromatism)
//! - Grid-based ray distribution
//! - Optical system assembly and propagation
//! - Ray tracing analysis and export
//!
//! Overall structure of the system:
//! 1. Create a scene (NodeGroup)
//! 2. Add a light source
//! 3. Define refractive index material model
//! 4. Create optical elements (lenses and aperture)
//! 5. Add elements into the scene
//! 6. Connect elements using distances
//! 7. Configure multi-wavelength ray tracing
//! 8. Run analysis and save output
use nalgebra::Point2;
use num::Zero;
use opossum_core::distributions::{energy::UniformDist, position::Grid, spectral::LaserLines};
use opossum_core::prelude::*;
use std::path::Path;
use uom::si::f64::Length;

/// Entry point of the example.
///
/// This function builds a Kepler telescope system and extends it by:
/// - generating rays at multiple wavelengths
/// - allowing observation of chromatic aberration effects
/// - exporting the system for visualization and analysis
fn main() -> OpmResult<()> {
    // Create a container for all optical elements in the system.
    // This acts as the scene (optical bench).
    let mut scenery = NodeGroup::new("Kepler chromatism");
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
    // Create the first spherical lens with modified curvature.
    // This helps emphasize chromatic effects.
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(130.0),
        millimeter!(-130.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    // Define a circular aperture to limit beam diameter.
    let aperture = Aperture::new_circle(
        millimeter!(25.0),
        ApertureType::Hole,
        Some(millimeter!(0.0, 0.0)),
    )?;
    // Attach the aperture to the input side of the first lens.
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    // Add the first lens to the optical scene.
    let i_pl1 = scenery.add_node(lens1)?;
    // Create the second spherical lens of the telescope.
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    // Add the second lens to the scene.
    let i_pl2 = scenery.add_node(lens2)?;
    // Add a ray propagation visualizer.
    // This only displays ray paths and does not affect optical behavior.
    let mut ray_prop_vis = RayPropagationVisualizer::new("after telescope", None)?;
    // Set visualization properties such as ray transparency.
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    // Add the visualizer to the scene.
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // Connect source → first lens (20 mm spacing).
    scenery.connect_nodes(i_src, "output_1", i_pl1, "input_1", millimeter!(20.0))?;
    // Connect first lens → second lens (125 mm spacing).
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    // Connect second lens → visualizer (50 mm spacing).
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(50.0))?;
    // Wrap the optical system into a document.
    // This stores the full setup for simulation or export.
    let mut doc = OpmDocument::new(scenery);
    // Define a multi-wavelength ray data source.
    // This enables simulation of chromatic aberration.
    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        // Define spatial distribution using a grid.
        Grid::new(
            Point2::new(Length::zero(), millimeter!(45.0)),
            Point2::new(1, 9),
        )?
        .into(),
        // Define uniform energy distribution for all rays.
        UniformDist::new(joule!(1.0))?.into(),
        // Define spectral distribution (two wavelengths).
        // This is key for observing chromatic dispersion.
        LaserLines::new(vec![(nanometer!(1000.0), 1.0), (nanometer!(350.0), 1.0)])?.into(),
    ));
    // Create default ray tracing configuration.

    let mut config = RayTraceConfig::default();
    // Map the custom ray source to the scene source node.
    config.map_source(i_src, ray_data_source.into());
    // Attach ray tracing analyzer.
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    // Save the optical system to file.
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_02_kepler_chromatism.opm",
    ))
}
