//! Kepler Imaging Field Example
//!
//! This example demonstrates how to construct a two-lens Keplerian optical
//! system for imaging a two-dimensional source using `opossum_core`. The
//! system traces rays through real lenses and uses fluence detectors to
//! analyze the image at multiple planes.
//!
//! System Overview
//! 1. One two-dimensional image source
//! 2. A fluence detector at the object plane
//! 3. Refractive index data for HZF52 glass
//! 4. One 75 mm spherical lens with a circular aperture
//! 5. One 50 mm spherical lens
//! 6. A fluence detector before the image plane
//! 7. A fluence detector at the image plane
//! 8. A fluence detector after the image plane
//! 9. A ray-tracing analyzer
//! 10. Serialization of the complete optical system for later visualization
//!     and analysis
//!
//! This example demonstrates:
//! - How to build a Keplerian optical system using two spherical lenses
//! - How to define wavelength-dependent refractive index data for HZF52 glass
//! - How to apply a circular aperture to the first lens
//! - How to connect optical components with physical propagation distances
//! - How to configure fluence detectors using the `Binning` estimator
//! - How to define a two-dimensional image as a ray source
//! - How to analyze the propagated image at multiple planes
//! - How to configure a ray-tracing analysis
//! - How to save the complete optical system as an `.opm` document
//!
//! Imports:
//! - `opossum_core::prelude::*` provides optical components, analyzers,
//!   unit macros, and document types
//! - `opossum_core::core_optics::{NodeAttrExt, OpticNodeExt,
//!   hit_map::fluence_estimator::FluenceEstimator}` provides optical node
//!   extensions and the fluence estimator used by the detectors
//! - `std::path::Path` is used to specify the input image and output
//!   document paths

use opossum_core::{
    core_optics::{NodeAttrExt, OpticNodeExt, hit_map::fluence_estimator::FluenceEstimator},
    prelude::*,
};
use std::path::Path;

/// Entry point for the program; returns `OpmResult<()>` to handle errors
fn main() -> OpmResult<()> {
    // 1. Define the two-dimensional image source.
    let mut scenery = NodeGroup::new("Kepler image field");
    let i_src = scenery.add_node(SourcePort::new("image source"))?;
    // 2. Define the fluence detector at the object plane.
    // The Binning estimator collects rays into spatial bins for analysis.
    let mut fluence_det = FluenceDetector::new("Object Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    // Add the object-plane fluence detector to the optical system.
    let i_sd5 = scenery.add_node(fluence_det)?;
    // 3. Define the wavelength-dependent refractive index for HZF52 glass
    // over the wavelength range from 300 nm to 2000 nm using the Schott model.
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    // 4. Define the first lens: 75 mm focal length and 10 mm thickness.
    // A circular aperture with a diameter of 25 mm is placed at its input.
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(130.0),
        millimeter!(-130.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let aperture = Aperture::new_circle(
        millimeter!(25.0),
        ApertureType::Hole,
        Some(millimeter!(0.0, 0.0)),
    )?;
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    let i_pl1 = scenery.add_node(lens1)?;
    // 5. Define the second lens: 50 mm focal length and 10 mm thickness.
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    // Add the second lens to the optical system.
    let i_pl2 = scenery.add_node(lens2)?;
    // 6. Define the fluence detector before the image plane.
    let mut fluence_det = FluenceDetector::new("Before Image Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_sd6 = scenery.add_node(fluence_det)?;
    // 7. Define the fluence detector at the image plane.
    let mut fluence_det = FluenceDetector::new("Image Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_sd7 = scenery.add_node(fluence_det)?;
    // 8. Define the fluence detector after the image plane.
    let mut fluence_det = FluenceDetector::new("After Image Plane");
    fluence_det.set_property("fluence estimator", FluenceEstimator::Binning.into())?;
    let i_sd8 = scenery.add_node(fluence_det)?;

    // 9. Connect the optical components.
    // The optical path is:
    // image source → object-plane detector → first lens → second lens →
    // detector before image plane → image-plane detector →
    // detector after image plane.
    scenery.connect_nodes(i_src, "output_1", i_sd5, "input_1", millimeter!(0.001))?;
    scenery.connect_nodes(i_sd5, "output_1", i_pl1, "input_1", millimeter!(70.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd6, "input_1", millimeter!(54.0))?;
    scenery.connect_nodes(i_sd6, "output_1", i_sd7, "input_1", millimeter!(4.0))?;
    scenery.connect_nodes(i_sd7, "output_1", i_sd8, "input_1", millimeter!(4.0))?;
    // 10. Configure the ray-tracing analysis.
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    // Define the two-dimensional image used as the ray source.
    let ray_data_source = RayDataSource::Image(ImageSrc::new(
        Path::new("../opossum_core/logo/Logo_square_tiny_grey_inverted.png").to_path_buf(),
        micrometer!(50.0),
        joule!(1.0),
        nanometer!(1000.0),
        degree!(1.0),
    )?);
    // Associate the image source with the source node.
    config.map_source(i_src, ray_data_source.into());
    // Add the configured ray-tracing analyzer to the document.
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    // Save the complete optical system and its analyzers to an OPM file.
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_05_kepler_imaging_field.opm",
    ))
}
