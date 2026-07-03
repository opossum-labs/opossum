//! PHELIX Main Amplifier Optical System Example
//! This example demonstrates a large optical system implemented in
//! `opossum_core` using `NodeGroup`, paraxial optical elements,
//! tilted mirrors, and inverted `NodeReference` nodes for reverse
//! ray propagation through existing optical paths.
//! The system represents a beam transport and multi-pass amplifier setup
//! inspired by high-energy laser systems.
//!
//! System Overview
//! 1. Point-source ray emitter for diagnostic ray generation
//! 2. Paraxial optical elements for beam transport
//! 3. Mirrors for changing beam direction
//! 4. Periscope mirror pair for vertical beam displacement
//! 5. Long focal-length relay optics
//! 6. Double-pass amplifier section implemented using `NodeReference`
//! 7. Ray propagation visualizer for inspecting ray trajectories
//! Distances between connected components are specified in millimeters.
//!
//! Principles and Objectives
//! This example demonstrates:
//! - Construction of large optical systems using hierarchical `NodeGroup` structures
//! - Beam transport using paraxial optical elements
//! - Beam steering using tilted mirrors
//! - Reuse of existing optical nodes using `NodeReference`
//! - Reverse propagation through existing optical elements using inverted references
//! - Multi-pass amplifier construction without duplicating optical geometry
//! - Ray-tracing analysis for imaging and collimation studies
//!
//! The amplifier section is implemented as a reusable optical subgraph.
//! An inverted `NodeReference` is used to propagate rays backward through the
//! same amplifier chain after reflection from a steering mirror.
//!
//! Import `opossum_core` modules:
//! - `prelude::*` provides optical system infrastructure, analyzers,
//!   unit macros, and optical components
//! - Distribution modules define spatial, spectral, and energy ray distributions
//! - `properties::Proptype` provides typed visualization properties
//!
//! External crates:
//! - `nalgebra::Point2` used for 2D source grid definition
//! - `nalgebra::Vector3` used for visualization direction vectors
//! - `uom` used for strongly typed physical units
//! - `num::Zero` used for zero-valued unit initialization

use nalgebra::Point2;
use nalgebra::Vector3;
use num::Zero;
use opossum_core::core_optics::NodeAttrExt;
use opossum_core::prelude::*;
use opossum_core::{
    distributions::{energy::UniformDist, position::Grid, spectral::LaserLines},
    properties::Proptype,
};
use std::env;
use std::path::Path;
use uom::si::f64::Length;

/// Entry point for the optical simulation.
///
/// The function constructs the complete optical system,
/// configures ray-tracing analyzers, and exports the
/// resulting simulation document.
///
/// Returns:
/// - `OpmResult<()>` indicating successful execution
///   or an optical-system error.
fn main() -> OpmResult<()> {
    // Create the top-level optical system container
    let mut scenery = NodeGroup::new("PHELIX MainAmp");
    // Add source node used for diagnostic ray injection
    let i_src = scenery.add_node(SourcePort::new("incoming rays DM"))?;

    // Initial transport section
    // Paraxial transport optics
    let i_l_pa_4_input = scenery.add_node(ParaxialSurface::new("T4 Input", millimeter!(931.0))?)?;
    let i_l_pa_4_exit = scenery.add_node(ParaxialSurface::new("T4 Exit", millimeter!(931.0))?)?;
    // Mirror for changing beam direction
    let i_m_pa_45 =
        scenery.add_node(ThinMirror::new("bridge_input").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    // Additional steering mirror
    let i_m_bridge =
        scenery.add_node(ThinMirror::new("bridge_input").with_tilt(degree!(0.0, 45.0, 0.0))?)?;

    // Long-distance transport optics
    let i_br_input =
        scenery.add_node(ParaxialSurface::new("bridge_input", millimeter!(2250.0))?)?;
    let i_br_exit = scenery.add_node(ParaxialSurface::new("bridge_exit", millimeter!(2250.0))?)?;
    // Periscope section
    // Upper periscope mirror
    let i_per_oben =
        scenery.add_node(ThinMirror::new("periscope_upper").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    // Lower periscope mirror
    let i_per_unten = scenery
        .add_node(ThinMirror::new("periscope_lower").with_tilt(degree!(-45.0, 0.0, 0.0))?)?;

    // Main transport optics
    // First relay optic
    let lens1 = ParaxialSurface::new("Input lens", millimeter!(1890.0))?;
    let i_l1 = scenery.add_node(lens1)?;
    // Mirrors for changing beam direction
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    let i_m2 = scenery.add_node(ThinMirror::new("mirror 2").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    // Long focal-length relay optic
    let lens2 = ParaxialSurface::new("Input lens", millimeter!(7590.0))?;
    let i_l2 = scenery.add_node(lens2)?;

    // Steering mirrors before the amplifier section
    let i_mm3 = scenery.add_node(ThinMirror::new("MM3").with_tilt(degree!(0.0, 45.0, 0.0))?)?;
    let i_mm2 = scenery.add_node(ThinMirror::new("MM2").with_tilt(degree!(0.0, 45.0, 0.0))?)?;

    // Reverse-propagation references
    // Create reverse-propagation reference for MM2
    let mut mm2_r = NodeReference::from_node(&scenery.node(i_mm2)?)?;
    mm2_r.set_inverted(true)?;
    let i_mm2_r = scenery.add_node(mm2_r)?;

    let mut mm3_r = NodeReference::from_node(&scenery.node(i_mm3)?)?;
    mm3_r.set_inverted(true)?;
    let i_mm3_r = scenery.add_node(mm3_r)?;

    // Create reverse-propagation reference for relay optic
    let mut l2_r = NodeReference::from_node(&scenery.node(i_l2)?)?;
    l2_r.set_inverted(true)?;
    let i_l2_r = scenery.add_node(l2_r)?;

    // Amplifier chain construction
    // Create amplifier-chain container
    let mut amps = NodeGroup::new("Amps");

    // Add amplifier stages
    let i_amp1 = amps.add_node(amp("Amp 1")?)?;
    let i_amp2 = amps.add_node(amp("Amp 2")?)?;
    let i_amp3 = amps.add_node(amp("Amp 3")?)?;
    let i_amp4 = amps.add_node(amp("Amp 4")?)?;
    let i_amp5 = amps.add_node(amp("Amp 5")?)?;
    let i_amp6 = amps.add_node(amp("Amp 6")?)?;

    // Connect amplifier stages
    amps.connect_nodes(i_amp1, "output", i_amp2, "input", millimeter!(880.0))?;
    amps.connect_nodes(i_amp2, "output", i_amp3, "input", millimeter!(880.0))?;
    amps.connect_nodes(i_amp3, "output", i_amp4, "input", millimeter!(880.0))?;
    amps.connect_nodes(i_amp4, "output", i_amp5, "input", millimeter!(880.0))?;
    amps.connect_nodes(i_amp5, "output", i_amp6, "input", millimeter!(880.0))?;

    // Map external ports
    amps.map_input_port(i_amp1, "input", "input")?;
    amps.map_output_port(i_amp6, "output", "output")?;

    // Double-pass amplifier section
    let mut main_amp = NodeGroup::new("Double-Pass Amps");

    // Add forward amplifier chain
    let i_amps = main_amp.add_node(amps)?;
    // Steering mirror for the return path
    let i_mm1 = main_amp.add_node(ThinMirror::new("MM1").with_tilt(degree!(0.0, 0.5, 0.0))?)?;
    // Create inverted reference for reverse propagation
    let mut amps_r = NodeReference::from_node(&main_amp.node(i_amps)?)?;
    amps_r.set_inverted(true)?;
    let i_amps_r = main_amp.add_node(amps_r)?;

    // Connect forward pass to steering mirror
    main_amp.connect_nodes(i_amps, "output", i_mm1, "input_1", millimeter!(1200.0))?;

    // Route rays through the inverted reference for the second pass
    main_amp.connect_nodes(i_mm1, "output_1", i_amps_r, "output", millimeter!(1200.0))?;
    // Map external ports
    main_amp.map_input_port(i_amps, "input", "input")?;
    main_amp.map_output_port(i_amps_r, "input", "output")?;

    // Add amplifier section to the main optical system
    let i_main_amp = scenery.add_node(main_amp)?;

    // Final transport optics
    // Final relay optic
    let lens3 = ParaxialSurface::new("Input lens", millimeter!(7590.0))?;
    let i_l3 = scenery.add_node(lens3)?;
    // Create ray propagation visualizer
    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    // Configure ray visualization transparency
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    // Configure visualization direction
    ray_prop_vis.set_property("view_direction", Proptype::Vec3(Vector3::y()))?;
    // Add visualizer node
    let i_sd3 = scenery.add_node(ray_prop_vis)?;

    // Connect optical system
    scenery.connect_nodes(
        i_src,
        "output_1",
        i_l_pa_4_input,
        "input_1",
        millimeter!(1310.0),
    )?;
    scenery.connect_nodes(
        i_l_pa_4_input,
        "output_1",
        i_l_pa_4_exit,
        "input_1",
        millimeter!(1862.0),
    )?;
    scenery.connect_nodes(
        i_l_pa_4_exit,
        "output_1",
        i_m_pa_45,
        "input_1",
        millimeter!(210.0),
    )?;
    scenery.connect_nodes(
        i_m_pa_45,
        "output_1",
        i_m_bridge,
        "input_1",
        millimeter!(1940.0),
    )?;
    scenery.connect_nodes(
        i_m_bridge,
        "output_1",
        i_br_input,
        "input_1",
        millimeter!(540.0),
    )?;
    scenery.connect_nodes(
        i_br_input,
        "output_1",
        i_br_exit,
        "input_1",
        millimeter!(4500.0),
    )?;
    scenery.connect_nodes(
        i_br_exit,
        "output_1",
        i_per_oben,
        "input_1",
        millimeter!(1000.0),
    )?; // originally 1250.0 mm
    scenery.connect_nodes(
        i_per_oben,
        "output_1",
        i_per_unten,
        "input_1",
        millimeter!(1660.0),
    )?;
    scenery.connect_nodes(
        i_per_unten,
        "output_1",
        i_l1,
        "input_1",
        millimeter!(1160.0),
    )?;
    scenery.connect_nodes(i_l1, "output_1", i_m1, "input_1", millimeter!(330.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(870.0))?; // originally 1120.0mm
    scenery.connect_nodes(i_m2, "output_1", i_l2, "input_1", millimeter!(8030.0))?;
    scenery.connect_nodes(i_l2, "output_1", i_mm3, "input_1", millimeter!(800.0))?;
    scenery.connect_nodes(i_mm3, "output_1", i_mm2, "input_1", millimeter!(2200.0))?;
    scenery.connect_nodes(i_mm2, "output_1", i_main_amp, "input", millimeter!(1070.0))?;

    // Reverse-propagation connections
    scenery.connect_nodes(i_main_amp, "output", i_mm2_r, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_mm2_r, "input_1", i_mm3_r, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_mm3_r, "input_1", i_l2_r, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_l2_r, "input_1", i_l3, "input_1", millimeter!(15180.0))?;

    scenery.connect_nodes(i_l3, "output_1", i_sd3, "input_1", millimeter!(800.0))?;

    // Create simulation document

    let mut doc = OpmDocument::new(scenery);

    // Imaging analysis
    // Configure ray-tracing analyzer
    let mut config = RayTraceConfig::default();
    // Structured point-source sampling
    let ray_data_source = RayDataSource::PointSrc(PointSrc::new(
        Grid::new(
            Point2::new(millimeter!(60.0), Length::zero()),
            Point2::new(5, 1),
        )?
        .into(),
        UniformDist::new(joule!(1.0))?.into(),
        LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        millimeter!(1000.0),
    )?);
    // Map source to optical system
    config.map_source(i_src, ray_data_source.into());
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Collimation analysis
    let mut config = RayTraceConfig::default();
    // Generate collimated line-ray distribution
    let ray_data_source = collimated_line_ray_builder(millimeter!(60.0), joule!(1.0), 5)?;
    // Map source to optical system
    config.map_source(i_src, ray_data_source);
    // Add analyzer
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Read the output directory from the environment, fallback to playground
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_09_phelix.opm");

    // Save the complete optical system to a file.
    // This file can be reopened in the framework for visualization or analysis.
    doc.save_to_file(&out_path)
}

/// Construct a simplified amplifier stage.
///
/// The amplifier stage consists of two tilted wedge elements that
/// represent transmissive disk-like optical elements.
///
/// Parameters:
/// - `name`: amplifier group name
///
/// Returns:
/// - `NodeGroup` representing one amplifier stage
fn amp(name: &str) -> OpmResult<NodeGroup> {
    // Constant refractive-index material
    let fused_silica = RefrIndexConst::new(1.5)?;
    // Create amplifier node group
    let mut amp = NodeGroup::new(name);
    // First wedge element
    let disk_a = Wedge::new("disk A", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(0.0, -56.0, 0.0))?;
    let i_a1 = amp.add_node(disk_a)?;

    // Second wedge element
    let disk_a = Wedge::new("disk B", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(0.0, 56.0, 0.0))?;
    let i_a2 = amp.add_node(disk_a)?;

    // Connect wedge elements
    amp.connect_nodes(i_a1, "output_1", i_a2, "input_1", millimeter!(900.0))?;
    // Map external ports
    amp.map_input_port(i_a1, "input_1", "input")?;
    amp.map_output_port(i_a2, "output_1", "output")?;
    Ok(amp)
}
