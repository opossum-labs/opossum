//! Multi Path / Multi Source Optical System Example
//!
//! This example demonstrates a multi-source optical system using wavelength-selective
//! beam splitters and energy-based optical analysis.
//!
//! System Overview
//! 1. Two independent optical sources (multi-line and single-line spectra)
//! 2. Spectral monitoring using ideal spectrometers
//! 3. Two wavelength-selective beam splitters (BS1, BS2)
//! 4. Channelized output routing based on long-pass edge filter behavior
//! 5. Energy analysis across multiple wavelength components
//!
//! Principles and Objectives
//! Each source emits a wavelength-resolved optical energy distribution
//! (i.e., discrete wavelength bins carrying optical energy or power).
//!
//! The system models how wavelength-resolved optical energy propagates through
//! an optical network where:
//! - shorter wavelengths are partially reflected or redirected according to the edge filter transfer function,
//! - longer wavelengths are preferentially transmitted (long-pass behavior),
//! - and total optical energy is conserved under ideal, lossless component assumptions.
//! This example shows how optical energy is distributed in a graph-based system
//! when multiple independent sources interact with wavelength-selective components.
//! The goal is to demonstrate wavelength-dependent energy routing
//! and redistribution in a graph-based optical network.
//!
//! This example demonstrates:
//! - How wavelength-selective beam splitters separate optical energy into distinct paths
//! - How multiple independent sources propagate through a shared optical network
//! - How spectral filtering redistributes energy across output channels
//! - How spectrometers measure wavelength-resolved optical energy distributions
//! - How energy analysis tracks conservation and redistribution of optical energy
//!
//! Imports:
//! - `opossum_core::prelude::*` provides optical system components, sources,
//!   detectors, and unit macros
//! - `opossum_core::analyzers::energy::EnergyConfig` provides energy analysis tools
//! - `std::path::Path` is used for saving the simulation output file
use opossum_core::{analyzers::energy::EnergyConfig, prelude::*};
use std::path::Path;

/// Entry point of the optical simulation.
///
/// Returns:
/// - `OpmResult<()>` indicating successful execution
///   or an optical-system error.
fn main() -> OpmResult<()> {
    // Create the top-level optical system container
    let mut scenery = NodeGroup::new("Multi Path / Multi Source");
    // Primary optical source emitting multiple laser lines
    let i_src = scenery.add_node(SourcePort::new("multi line source"))?;
    // Spectrometer monitoring the primary source output
    let i_s1 = scenery.add_node(Spectrometer::new("Source 1", SpectrometerType::Ideal))?;

    // BS1: wavelength-dependent separation using long-pass filter at 980 nm
    let splitting_config_1 = SplittingConfigBuilder::Spectrum(
        EdgeFilter::new(
            EdgeFilterType::LongPass,
            nanometer!(980.0),
            (0.)..(1.),
            None,
            nanometer!(800.0)..nanometer!(1100.0),
            nanometer!(0.5),
        )?
        .into(),
    );
    // Create first wavelength-selective beam splitter
    let bs1 = BeamSplitter::new("BS1", &splitting_config_1)?;
    let i_bs1 = scenery.add_node(bs1)?;

    // Spectrometer monitoring BS1 output channel 1
    let i_s2 = scenery.add_node(Spectrometer::new("BS1 Output 1", SpectrometerType::Ideal))?;

    // Spectrometer monitoring BS1 output channel 2
    let i_s3 = scenery.add_node(Spectrometer::new("BS1 Output 2", SpectrometerType::Ideal))?;

    // BS2: secondary wavelength-selective beam splitter
    // Long-pass edge filter with cutoff wavelength at 1025 nm
    let splitting_config_2 = SplittingConfigBuilder::Spectrum(
        EdgeFilter::new(
            EdgeFilterType::LongPass,
            nanometer!(1025.0),
            (0.)..(1.),
            None,
            nanometer!(800.0)..nanometer!(1100.0),
            nanometer!(0.5),
        )?
        .into(),
    );
    // Create second beam splitter stage
    let bs2 = BeamSplitter::new("BS2", &splitting_config_2)?;
    let i_bs2 = scenery.add_node(bs2)?;

    // Secondary optical source: single-line spectral input injected into BS2
    let i_src2 = scenery.add_node(SourcePort::new("source 2"))?;
    // Spectrometer monitoring BS2 output channel 1
    let i_s4 = scenery.add_node(Spectrometer::new("BS2 Output 1", SpectrometerType::Ideal))?;
    // Spectrometer monitoring BS2 output channel 2
    let i_s5 = scenery.add_node(Spectrometer::new("BS2 Output 2", SpectrometerType::Ideal))?;

    // Optical System Connections
    // Connect primary source to spectrometer
    scenery.connect_nodes(i_src, "output_1", i_s1, "input_1", millimeter!(100.0))?;
    // Connect spectrometer output to BS1 input
    scenery.connect_nodes(i_s1, "output_1", i_bs1, "input_1", millimeter!(100.0))?;

    // Beam splitter outputs:
    //
    // Output channel 1
    // - out1_trans1_refl2:
    // output channel defined by beam splitter port naming convention
    // (combination of transmission and reflection pathways)
    scenery.connect_nodes(
        i_bs1,
        "out1_trans1_refl2",
        i_s2,
        "input_1",
        millimeter!(100.0),
    )?;
    // Output channel 2
    // - out2_trans2_refl1:
    // transmitted light from input 2 and reflected light from input 1
    scenery.connect_nodes(
        i_bs1,
        "out2_trans2_refl1",
        i_s3,
        "input_1",
        millimeter!(100.0),
    )?;

    // Route BS1 output channel 1 (via spectrometer) into BS2 input channel 1
    scenery.connect_nodes(i_s2, "output_1", i_bs2, "input_1", millimeter!(100.0))?;
    // Feed second source into BS2 input channel 2
    scenery.connect_nodes(i_src2, "output_1", i_bs2, "input_2", millimeter!(100.0))?;

    scenery.connect_nodes(
        i_bs2,
        "out1_trans1_refl2",
        i_s4,
        "input_1",
        millimeter!(100.0),
    )?;

    // Connect BS2 output channel 2 to spectrometer
    scenery.connect_nodes(
        i_bs2,
        "out2_trans2_refl1",
        i_s5,
        "input_1",
        millimeter!(100.0),
    )?;

    // Create simulation document from optical graph
    let mut doc = OpmDocument::new(scenery);
    // Energy distribution for first source:
    // multiple discrete laser wavelengths with different energies
    let energy_data_builder_1 = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![
            (nanometer!(1000.0), joule!(0.5)),
            (nanometer!(950.0), joule!(0.3)),
            (nanometer!(1050.0), joule!(0.2)),
        ],
        nanometer!(0.5),
    )?);

    // Energy distribution for second source:
    // single dominant laser wavelength
    let energy_data_builder_2 = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
        vec![(nanometer!(1020.0), joule!(1.0))],
        nanometer!(0.5),
    )?);
    // Configure wavelength-resolved optical energy analysis
    let mut config = EnergyConfig::default();
    // Map sources into energy analysis model
    config.map_source(i_src, energy_data_builder_1.into());
    config.map_source(i_src2, energy_data_builder_2.into());
    // Attach analyzer to document
    doc.add_analyzer(AnalyzerType::Energy(config));
    // Save simulation output to OPM file
    doc.save_to_file(Path::new(
        "./opossum_core/playground/workshop_10_multi_path.opm",
    ))
}
