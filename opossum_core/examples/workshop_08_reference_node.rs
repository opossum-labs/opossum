//! Reference Node Optical System Example
//! This example demonstrates an optical system in `opossum_core` where a lens
//! is referenced through a `NodeReference`, then inverted to enable reverse propagation through the reference node.
//! A point ray source with structured spatial sampling is used.
//!
//! System Overview
//! 1. Grid-sampled point ray source
//! 2. 75 mm lens as primary optical element
//! 3. Thin mirror with a 0.5° tilt angle
//! 4. NodeReference linked to the first lens, with inverted propagation enabled
//! 5. Ray propagation visualizer
//! Distances between components are specified in millimeters.
//!
//! Principles and Objectives
//! The system uses a simulation graph that includes a `NodeReference` node.
//! It also demonstrates inversion of the reference node to route rays back through the original optical element.
//! This setup lets you:
//! - Use NodeReference to reference an existing optical node in the system graph
//! - Invert propagation direction through a referenced optical element
//! - Combine lens, mirror, and reference node in a single optical graph path with reversed propagation through the reference node
//! - Generate rays using structured spatial, energy, and spectral definitions
//!
//! Import `opossum_core` modules:
//! - `prelude::*` provides core optical system types, unit macros (millimeter!, degree!, nanometer!, joule!), and optical components (sources, lenses, mirrors, detectors)
//! - Distribution modules define ray generation using spatial grid (`Grid`), energy distribution (`UniformDist`), and spectral lines (`LaserLines`)
//!
//! External crates:
//! - `nalgebra::Point2` used to define 2D coordinates for constructing the spatial emission grid
use nalgebra::Point2;
use opossum_core::distributions::{energy::UniformDist, position::Grid, spectral::LaserLines};
use opossum_core::prelude::*;
use std::path::Path;
// Entry point for the simulation; returns `OpmResult<()>` for safe execution
fn main() -> OpmResult<()> {
    // Create a default optical system container
    let mut scenery = NodeGroup::default();
    // Add a point ray source node
    let i_src = scenery.add_node(SourcePort::new("point ray source"))?;
    // Define wavelength-dependent refractive index for HZF52 glass using Schott model
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    // Create primary 75 mm lens
    let lens1 = Lens::new(
        "75 mm lens",
        millimeter!(122.25),
        millimeter!(-122.25),
        millimeter!(5.0),
        &refr_index_hzf52,
    )?;
    // Add lens to system
    let i_l1 = scenery.add_node(lens1)?;
    // Create a thin mirror with a 0.5° tilt angle
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(0.5, 0.0, 0.0))?)?;
    // Create a reference copy of the first lens
    let mut l1_ref = NodeReference::from_node(&scenery.node(i_l1)?);
    // Invert propagation direction of referenced lens
    l1_ref.set_inverted(true)?;
    // Add referenced lens to system
    let i_l1_ref = scenery.add_node(l1_ref)?;
    // Create ray propagation visualizer
    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    // Add visualizer node
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // Connect optical system with defined spacing
    scenery.connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(75.5))?;
    scenery.connect_nodes(i_l1, "output_1", i_m1, "input_1", millimeter!(40.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_l1_ref, "output_1", millimeter!(40.0))?;
    scenery.connect_nodes(i_l1_ref, "input_1", i_sd3, "input_1", millimeter!(75.5))?;
    // The system is implemented as a simulation graph using `NodeReference`.
    let mut doc = OpmDocument::new(scenery);
    // Define structured point source with spatial grid sampling
    let ray_data_source = RayDataSource::PointSrc(PointSrc::new(
        Grid::new(
            Point2::new(millimeter!(0.0), millimeter!(5.0)),
            Point2::new(1, 5),
        )?
        .into(),
        UniformDist::new(joule!(1.0))?.into(),
        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        millimeter!(75.0),
    )?);
    // Default ray tracing configuration
    let mut config = RayTraceConfig::default();
    // Map structured source to optical system
    config.map_source(i_src, ray_data_source.into());
    // Add ray tracing analyzer
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    // Save simulation setup to file
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_08_reference_node.opm",
    ))
}
