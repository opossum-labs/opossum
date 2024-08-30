#![warn(missing_docs)]
//! Optical Analyzers
//!
//! An analyzer of a certain [`AnalyzerType`] determines how an (`OpticScenery`)[`crate::OpticScenery`] is analyzed. For example, the energy flow for a scenery can be
//! calculated as a simple analysis. On the other hand, a full Fourier propagation could be performed. The result of an analysis run can be written to a JSON structure
//! and / or exported as a PDF report.
use std::fmt::Display;

use strum::EnumIter;
pub mod energy;
pub mod ghostfocus;
pub mod raytrace;

pub use ghostfocus::GhostFocusConfig;
pub use raytrace::RayTraceConfig;

use crate::{error::OpmResult, OpticScenery};

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
    /// Ghost focus analysis.
    ///
    /// This mode also performs ray tracing but considers parasitic back reflections from surfaces wtih a given number of bounces.
    GhostFocus(GhostFocusConfig),
}

impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Energy => "energy",
            Self::RayTrace(_) => "ray tracing",
            Self::GhostFocus(_) => "ghost focus analysis",
        };
        write!(f, "{msg}")
    }
}
#[cfg(test)]
mod test {
    use super::*;
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
}

/// Marker trait for all Analyzers
pub trait Analyzer {
    /// Analyze an [`OpticScenery`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the concrete implementation of the [`Analyzer`] returns an error.
    fn analyze(&self, _scenery: &mut OpticScenery) -> OpmResult<()>;
    /// Generate an analysis report
    fn report(&self) {}
}
