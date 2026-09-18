//! Kepler Imaging Point Source Example
//!
//! This example demonstrates how to construct a Keplerian optical system
//! for imaging a point source using `opossum_core`. The system includes
//! a point light source, two lenses, a ray propagation visualizer,
//! and spot diagram analyzers.
//!
//! System Overview
//! 1. One point light source
//! 2. Refractive index data for HZF52 glass
//! 3. One 75 mm spherical lens with a circular aperture
//! 4. One 50 mm spherical lens
//! 5. A ray propagation visualizer
//! 6. Two spot diagram analyzers
//! 7. A point-source ray-tracing configuration
//! 8. A ray-tracing analyzer
//! 9. Serialization of the complete optical system for later visualization
//!    and analysis
//!
//! This example demonstrates:
//! - How to build a Keplerian optical system using two spherical lenses
//! - How to define wavelength-dependent refractive index data
//! - How to apply a circular aperture to a lens input
//! - How to connect optical components with physical propagation distances
//! - How to use spot diagram analyzers to inspect ray distributions
//! - How to visualize ray propagation through the optical system
//! - How to configure a point-source ray-tracing analysis
//! - How to save the complete optical system as an `.opm` document
//!
//! Imports:
//! - `opossum_core::prelude::*` provides optical components, analyzers,
//!   unit macros, and document types
//! - `opossum_core::core_optics::{NodeAttrExt, OpticNodeExt}` provides
//!   extension traits for optical node attributes and properties
//! - `opossum_core::distributions::*` provides spatial, energy, and
//!   spectral distributions for the point source
//! - `std::{env, path::Path}` is used to determine the output directory
//!   and save the generated `.opm` file

use opossum_core::core_optics::{NodeAttrExt, OpticNodeExt};
use opossum_core::distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines};
use opossum_core::prelude::*;
use std::env;
use std::path::Path;

fn main() -> OpmResult<()> {
    // 1. Define the point light source.
    let mut scenery = NodeGroup::new("Kepler imaging point src");
    let i_src = scenery.add_node(SourcePort::new("point source"))?;
    // 2. Define the wavelength-dependent refractive index for HZF52 glass
    // over the wavelength range from 300 nm to 2000 nm.
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    // 3. Define the first lens: 75 mm focal length, 10 mm thickness,
    // with a circular aperture of 25 mm.
    let mut lens1 = Lens::new(
        "75 mm lens",
        millimeter!(130.0),
        millimeter!(-130.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let aperture = Aperture::new_circle(millimeter!(25.0), ApertureType::Hole, None)?;
    lens1.set_aperture(&PortType::Input, "input_1", &aperture)?;
    let i_pl1 = scenery.add_node(lens1)?;
    // 4. Define the second lens: 50 mm focal length and 10 mm thickness.
    let lens2 = Lens::new(
        "50 mm lens",
        millimeter!(100.0),
        millimeter!(-100.0),
        millimeter!(10.0),
        &refr_index_hzf52,
    )?;
    let i_pl2 = scenery.add_node(lens2)?;
    // 5. Define the ray propagation visualizer.
    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;
    // 6. Define the spot diagram analyzers.
    let i_sd4 = scenery.add_node(SpotDiagram::new("spot at source")?)?;
    let i_sd5 = scenery.add_node(SpotDiagram::new("spot at image")?)?;
    // 7. Connect the optical components.
    // The optical path is:
    // point source → source spot diagram → first lens → second lens →
    // ray propagation visualizer → image spot diagram.
    scenery.connect_nodes(i_src, "output_1", i_sd4, "input_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_sd4, "output_1", i_pl1, "input_1", millimeter!(70.0))?;
    scenery.connect_nodes(i_pl1, "output_1", i_pl2, "input_1", millimeter!(125.0))?;
    scenery.connect_nodes(i_pl2, "output_1", i_sd3, "input_1", millimeter!(58.0))?;
    scenery.connect_nodes(i_sd3, "output_1", i_sd5, "input_1", millimeter!(0.1))?;
    // 8. Configure the point-source ray-tracing analysis.
    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(
        i_src,
        RayDataSource::PointSrc(PointSrc::new(
            Hexapolar::new(millimeter!(15.0), 8)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
            millimeter!(70.0),
        )?)
        .into(),
    );
    // 9. Add the configured ray-tracing analyzer to the document.
    doc.add_analyzer(AnalyzerType::RayTrace(config));

    // Read the output directory from the environment, falling back to the playground directory.
    let out_dir = env::var("OPOSSUM_EXAMPLES_OUT_DIR")
        .unwrap_or_else(|_| "./opossum_core/playground".to_string());
    let out_path = Path::new(&out_dir).join("workshop_04_kepler_imaging_point.opm");

    // Save the complete optical system to an OPM file.
    // The saved file can later be opened in the OPOSSUM framework for
    // visualization and analysis.
    doc.save_to_file(&out_path)
}
