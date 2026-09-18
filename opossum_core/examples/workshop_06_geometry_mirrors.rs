//! Geometry Mirror System Example
//!
//! This example demonstrates a simple two-mirror optical system using
//! `opossum_core`. A collimated ray source is reflected by two tilted mirrors,
//! including a curved mirror, and the resulting ray propagation is visualized.
//!
//! System Overview
//! 1. One collimated line ray source
//! 2. One flat mirror with 50% constant reflectivity
//! 3. One curved mirror with opposite tilt
//! 4. One ray propagation visualizer
//! 5. A ray-tracing analyzer
//! 6. Serialization of the complete optical system for later visualization
//!    and analysis
//!
//! This example demonstrates:
//! - How to create and configure flat and curved mirrors
//! - How to set mirror tilt angles
//! - How to assign a constant-reflectivity coating to a mirror
//! - How to define mirror curvature
//! - How to connect optical components with physical propagation distances
//! - How to configure a ray propagation visualizer
//! - How to configure a ray-tracing analyzer
//! - How to save the complete optical system as an `.opm` document
//!
//! Imports:
//! - `opossum_core::coatings::CoatingConstantR` provides the
//!   constant-reflectivity coating model
//! - `opossum_core::core_optics::{NodeAttrExt, OpticNodeExt}` provides
//!   optical node extension traits
//! - `opossum_core::{percent, prelude::*}` provides the percent macro,
//!   optical components, analyzers, unit macros, and document types
//! - `std::{env, path::Path}` is used to determine the output directory
//!   and save the generated `.opm` file

use opossum_core::coatings::CoatingConstantR;
use opossum_core::core_optics::{NodeAttrExt, OpticNodeExt};
use opossum_core::{percent, prelude::*};
use std::env;
// Import `Path` from the standard library for working with file paths
use std::path::Path;
// Entry point for the program; returns `OpmResult<()>` to handle errors
fn main() -> OpmResult<()> {
    // 1. Define the collimated line ray source.
    let mut scenery = NodeGroup::new("Geometry, mirror system");
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;
    // 2. Define the first flat mirror with a tilt of 22.5 degrees.
    // Assign a constant-reflectivity coating with 50% reflectivity.
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(22.5, 0.0, 0.0))?;
    mirror1.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingConstantR::new(percent!(50.0))?.into(),
    )?;
    // Add the first mirror to the optical system.
    let i_m1 = scenery.add_node(mirror1)?;
    // 3. Define the second mirror with a curvature of -100 mm
    // and a tilt of -22.5 degrees.
    let i_m2 = scenery.add_node(
        ThinMirror::new("mirror 2")
            .with_curvature(millimeter!(-100.0))?
            .with_tilt(degree!(-22.5, 0.0, 0.0))?,
    )?;
    // 4. Define the ray propagation visualizer.
    let mut ray_prop_vis = RayPropagationVisualizer::default();
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_prop_vis = scenery.add_node(ray_prop_vis)?;

    // 5. Connect the optical components.
    // The optical path is:
    // collimated source → mirror 1 → mirror 2 → ray propagation visualizer.
    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_prop_vis, "input_1", millimeter!(80.0))?;

    // 6. Configure the ray-tracing analysis.
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    // Define a collimated line ray source with a 20 mm beam width,
    // 1 joule of energy per ray, and 9 rays.
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(20.0), joule!(1.0), 9)?,
    );
    // Add the ray-tracing analyzer to the document.
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Read the output directory from the environment, fallback to playground.
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_06_geometry_mirrors.opm");

    // Save the complete optical system and its analyzer as an OPM file.
    doc.save_to_file(&out_path)
}