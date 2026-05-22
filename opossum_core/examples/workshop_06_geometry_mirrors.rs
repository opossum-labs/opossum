//! Geometry Mirror System Example
//!
//! This example demonstrates a simple two-mirror optical system using
//! `opossum_core`. Rays from a collimated source reflect from tilted mirrors
//! and are visualized after propagation.
//!
//! System Overview
//! 1. Collimated line ray source
//! 2. Flat mirror with partial reflectivity
//! 3. Curved mirror tilted in the opposite direction
//! 4. Ray propagation visualizer
//!
//! Distances between components are specified in millimeters.
//!
//! Principles and Objectives
//!
//! Mirrors redirect light depending on their tilt and curvature.
//! This setup lets you:
//! - Observe how tilted mirrors steer rays.
//! - Understand how curved mirrors affect propagation.
//! - Visualize ray reflection and beam direction.
//! - Learn how optical nodes are connected in OPOSSUM.
//!
//! Import `opossum_core` modules:
//! - `CoatingType` for mirror reflectivity properties
//! - `prelude::*` brings in core optical system types and helpers
//! 
use opossum_core::coatings::CoatingType;
use opossum_core::prelude::*;
// Import `Path` from the standard library for working with file paths
use std::path::Path;
// Entry point for the program; returns `OpmResult<()>` to handle errors
fn main() -> OpmResult<()> {
    // Create a new optical system container named "Geometry, mirror system"
    let mut scenery = NodeGroup::new("Geometry, mirror system");
    // Add a source node representing a collimated line ray source
    let i_src = scenery.add_node(SourcePort::new("collimated line ray source"))?;
    // Create the first mirror tilted by 22.5 degrees
    let mut mirror1 = ThinMirror::new("mirror 1").with_tilt(degree!(22.5, 0.0, 0.0))?;
    // Apply a partially reflective coating with 50% reflectivity
    mirror1.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingType::ConstantR { reflectivity: 0.5 },
    )?;
    // Add the first mirror to the optical system
    let i_m1 = scenery.add_node(mirror1)?;
    // Create the second mirror with curvature and opposite tilt
    let i_m2 = scenery.add_node(
        ThinMirror::new("mirror 2")
            .with_curvature(millimeter!(-100.0))?
            .with_tilt(degree!(-22.5, 0.0, 0.0))?,
    )?;
    // Create a ray propagation visualizer
    let mut ray_prop_vis = RayPropagationVisualizer::default();
   // Set ray transparency property for clear visualization
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    // Add the visualizer to the system
    let i_prop_vis = scenery.add_node(ray_prop_vis)?;
     // Connect source → mirror 1
    scenery.connect_nodes(i_src, "output_1", i_m1, "input_1", millimeter!(100.0))?;
    // Connect mirror 1 → mirror 2
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(100.0))?;
    // Connect mirror 2 → ray visualizer
    scenery.connect_nodes(i_m2, "output_1", i_prop_vis, "input_1", millimeter!(80.0))?;
    // Create a document object for ray tracing
    let mut doc = OpmDocument::new(scenery);
    // Default configuration for ray tracing
    let mut config = RayTraceConfig::default();
    // Define a collimated line ray source:
    // - 20 mm beam width
    // - 1 Joule energy
    // - 9 rays
    config.map_source(
        i_src,
        collimated_line_ray_builder(millimeter!(20.0), joule!(1.0), 9)?,
    );
    // Add ray tracing analysis to the document
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    // Save the configured optical system and analyzers to a file
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_06_geometry_mirrors.opm",
    ))
}
