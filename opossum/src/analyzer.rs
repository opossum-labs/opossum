#![warn(missing_docs)]
//! Optical Analyzers
//!
//! An analyzer of a certain [`AnalyzerType`] determines how an (`OpticScenery`)[`crate::OpticScenery`] is analyzed. For example, the energy flow for a scenery can be
//! calculated as a simple analysis. On the other hand, a full Fourier propagation could be performed. The result of an analysis run can be written to a JSON structure
//! and / or exported as a PDF report.
use std::fmt::Display;
use strum::EnumIter;
use uom::si::f64::Energy;

/// Type of analysis to be performed.
#[non_exhaustive]
#[derive(EnumIter, PartialEq, Debug)]
pub enum AnalyzerType {
    /// Simple energy flow of an optical spectrum.
    ///
    /// **Note**: This mode does consider any geometric aspects of an optical setup. In particular, possible apertures of optical elements are ignored.
    Energy,
    /// Ray tracing analysis.
    ///
    /// This mode simulates a bundle of optical ray propagating through a scenery.
    RayTrace(RayTraceConfig),
}

#[derive(Default, Debug, PartialEq)]
enum RayTracingMode {
    #[default]
    /// Sequential mode
    ///
    /// In this mode, rays follow the directed graph from node to node. If the next node is not hit, further propagation is dropped. This mode is
    /// mostly useful for imaging, collimation, and optimizing of "simple" optical lens systems.
    Sequential,
    // /// Semi-sequential mode
    // ///
    // /// Rays may bounce and traverse the graph in backward direction. If the next intended node is not hit, further propagation is dropped.
    // /// Interesting for ghost focus simulation
    // SemiSequential,
    // /// Non-sequential mode
    // ///
    // /// Rays do not follow a specific direction of the graph. Skipping of nodes may be allowed. Interesting for stray-light analysis, flash-lamp pumping, beam dumps, etc.
    // NonSequential
}

#[derive(Default, PartialEq, Debug)]
/// Configuration data for a rays tracing analysis.
///
/// It currently only contains the `RayTracingMode`.
pub struct RayTraceConfig {
    mode: RayTracingMode,
    min_energy_per_ray: Energy,
}
impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Energy => "energy",
            Self::RayTrace(_) => "ray tracing",
        };
        write!(f, "{msg}")
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use assert_matches::assert_matches;
    #[test]
    fn display() {
        assert_eq!(format!("{}", AnalyzerType::Energy), "energy");
        assert_eq!(
            format!("{}", AnalyzerType::RayTrace(RayTraceConfig::default())),
            "ray tracing"
        );
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", AnalyzerType::Energy), "Energy");
    }
    #[test]
    fn ray_tracing_mode_default() {
        assert_matches!(RayTracingMode::default(), RayTracingMode::Sequential);
    }
    #[test]
    fn ray_tracing_mode_debug() {
        assert_eq!(format!("{:?}", RayTracingMode::default()), "Sequential");
    }
    #[test]
    fn ray_tracing_config_default() {
        assert_matches!(RayTraceConfig::default().mode, RayTracingMode::Sequential);
    }
    #[test]
    fn ray_tracing_config_debug() {
        assert_eq!(
            format!("{:?}", RayTraceConfig::default()),
            "RayTraceConfig { mode: Sequential }"
        );
    }
}
