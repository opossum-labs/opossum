//! Kepler Imaging Field Example
//!
//! This example demonstrates a two-lens Keplerian system imaging a 2D source 
//! using `opossum_core`. The system traces rays through real lenses 
//! and analyzes image formation at multiple planes using fluence detectors.
//!
//! System Overview
//! 1. Image source (2D pattern)
//! 2. First lens (75 mm focal length, HZF52 glass) with circular aperture
//! 3. Second lens (50 mm focal length, HZF52 glass)
//! 4. Fluence detectors before, at, and after the image plane
//!
//! Distances between components are specified in millimeters.
//! Principles and Objectives
//!
//! Unlike ideal lenses, real lenses introduce aberrations (spherical, chromatic) 
//! that blur images. This setup lets you:
//! - Visualize how a 2D image propagates through a lens system.
//! - Observe image quality at different planes using fluence detectors.
//! - Understand how lens spacing, aperture, and focal lengths affect focus.
//! 
//! Import `opossum_core` modules:
//! - `FluenceEstimator` for defining how detectors measure light
//! - `prelude::*` brings in the core optical system types and helpers
use opossum_core::{core_optics::hit_map::fluence_estimator::FluenceEstimator, prelude::*};
//! Import `Path` from the standard library for working with file paths
use std::path::Path;
//! Entry point for the program; returns `OpmResult<()>` to handle errors

fn main() -> OpmResult<()> {
 // Create a new optical system container named "Kepler image field"
    let mut scenery = NodeGroup::new("Kepler image field");
// Add a source node representing the 2D image input
    let i_src = scenery.add_node(SourcePort::new("image source"))?;
 // Create a fluence detector at the object plane
    let mut fluence_det = FluenceDetector::new("Object Plane");
// Set the detector to use "Binning" to collect rays into bins for analysis
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
//  Add the object plane fluence detector to the system
    let i_sd5 = scenery.add_node(fluence_det)?;
 // Define the refractive index for HZF52 glass over 300–2000 nm using the Schott model
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
 // Create the first lens: 75 mm focal length, 10 mm thick
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(130.0),
        millimeter!(-130.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
// Define a circular aperture of 25 mm at the lens center
    let aperture =
        Aperture::new_circle(millimeter!(25.0), millimeter!(0., 0.), ApertureType::Hole)?;
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    let i_pl1 = scenery.add_node(lens1)?;
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
// Add the second lens to the optical system
    let i_pl2 = scenery.add_node(lens2)?;
// Fluence detector to measure light just before the image plane
    let mut fluence_det = FluenceDetector::new("Before Image Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_sd6 = scenery.add_node(fluence_det)?;
 // Add the detector to the system
    let mut fluence_det = FluenceDetector::new("Image Plane");
// Use Binning for ray collection
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
// Add the image plane detector
    let i_sd7 = scenery.add_node(fluence_det)?;
// Detector placed after the image plane
    let mut fluence_det = FluenceDetector::new("After Image Plane");
// Use Binning for measurement
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
 // Add it to the system
    let i_sd8 = scenery.add_node(fluence_det)?;
// Connect source → object plane detector
   
    scenery.connect_nodes(i_src, "output_1", i_sd5, "input_1", millimeter!(0.001))?;
    // Object plane → first lens
    scenery.connect_nodes(i_sd5, "output_1", i_pl1, "input_1", millimeter!(70.0))?;
   // First lens → second lens
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    // Second lens → detector before image plane
    scenery.connect_nodes(i_pl2, "output_1", i_sd6, "input_1", millimeter!(54.0))?;
    // Detector before image plane → image plane detector
    scenery.connect_nodes(i_sd6, "output_1", i_sd7, "input_1", millimeter!(4.0))?;
    // Image plane detector → detector after image plane
    scenery.connect_nodes(i_sd7, "output_1", i_sd8, "input_1", millimeter!(4.0))?;
    // Create a document object for ray tracing

    let mut doc = OpmDocument::new(scenery);
    // Default configuration for ray tracing
    let mut config = RayTraceConfig::default();
     // Define the image to be traced as rays
    let ray_data_source = RayDataSource::Image(ImageSrc::new(
        Path::new("../opossum_core/logo/Logo_square_tiny_grey_inverted.png").to_path_buf(),
        micrometer!(50.0),
        joule!(1.0),
        nanometer!(1000.0),
        degree!(1.0),
    )?);
    // Assign the ray source to the source node
    config.map_source(i_src, ray_data_source.into());
    // Add ray tracing analysis to the document
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    // Save the configured optical system and analyzers to a file
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_05_kepler_imaging_field.opm",
    ))
}
