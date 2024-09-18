#![warn(missing_docs)]
//! Optical Analyzers
//!
//! An analyzer of a certain [`AnalyzerType`] determines how a [`NodeGroup`](`crate::nodes::NodeGroup`) is analyzed.
//! For example, the energy flow for a scenery can be calculated as a simple analysis. On the other hand, a full
//! Fourier propagation could be performed. The result of an analysis run can be written to a JSON structure
//! and / or exported as a PDF report.
pub mod energy;
pub mod ghostfocus;
pub mod raytrace;

use crate::{error::OpmResult, nodes::NodeGroup};
pub use ghostfocus::GhostFocusConfig;
pub use raytrace::RayTraceConfig;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;

/// Type of analysis to be performed.
#[non_exhaustive]
#[derive(EnumIter, PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum AnalyzerType {
    /// Simple energy flow analysis of an optical spectrum.
    ///
    /// **Note**: This mode does not consider any geometric aspects of an optical setup (so far). In particular,
    /// possible apertures of optical elements are ignored.
    Energy,
    /// Ray tracing analysis.
    ///
    /// This mode simulates a bundle of optical ray propagating through a scenery.
    RayTrace(RayTraceConfig),
    /// Ghost focus analysis.
    ///
    /// This mode also performs ray tracing but considers parasitic back reflections from surfaces wtih a
    /// given number of bounces.
    GhostFocus(GhostFocusConfig),
}
impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Energy => "energy",
            Self::RayTrace(_) => "ray tracing",
            Self::GhostFocus(_) => "ghost focus",
        };
        write!(f, "{msg} analysis")
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn display() {
        assert_eq!(format!("{}", AnalyzerType::Energy), "energy analysis");
        assert_eq!(
            format!("{}", AnalyzerType::RayTrace(RayTraceConfig::default())),
            "ray tracing analysis"
        );
        assert_eq!(
            format!("{}", AnalyzerType::GhostFocus(GhostFocusConfig::default())),
            "ghost focus analysis"
        );
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", AnalyzerType::Energy), "Energy");
    }
}

/// Marker trait for all Analyzers
pub trait Analyzer {
    /// Analyze a [`NodeGroup`].
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the [`Analyzer`] returns an error.
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()>;
}
