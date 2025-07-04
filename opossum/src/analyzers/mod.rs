#![warn(missing_docs)]
//! Optical Analyzers
//!
//! An analyzer of a certain [`AnalyzerType`] determines how a [`NodeGroup`](`crate::nodes::NodeGroup`) is analyzed.
//! For example, the energy flow for a scenery can be calculated as a simple analysis. On the other hand, a full
//! Fourier propagation could be performed. The result of an analysis run can be written to a JSON structure
//! and / or exported as a PDF report.
pub mod analyzable;
pub mod energy;
pub mod ghostfocus;
pub mod raytrace;

use crate::{error::OpmResult, nodes::NodeGroup, reporting::analysis_report::AnalysisReport};
pub use analyzable::Analyzable;
pub use ghostfocus::GhostFocusConfig;
pub use raytrace::RayTraceConfig;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::EnumIter;
use strum::IntoEnumIterator;
use utoipa::ToSchema;

/// Type of analysis to be performed.
///
/// While the individual analyzers are implemented as traits, this enum is necessary for serialization / desrialization.
#[non_exhaustive]
#[derive(EnumIter, PartialEq, Debug, Serialize, Deserialize, Clone, ToSchema)]
pub enum AnalyzerType {
    /// Simple energy flow analysis of an optical spectrum.
    ///
    /// **Note**: This mode does not consider any geometric aspects of an optical setup (so far). In particular,
    /// possible apertures of optical elements are ignored.
    Energy,
    /// Ray tracing analysis.
    ///
    /// This mode simulates a bundle of optical ray propagating through a scenery.
    #[schema(value_type=())]
    RayTrace(RayTraceConfig),
    /// Ghost focus analysis.
    ///
    /// This mode also performs ray tracing but considers parasitic back reflections from surfaces wtih a
    /// given number of bounces.
    #[schema(value_type=())]
    GhostFocus(GhostFocusConfig),
}
impl AnalyzerType {
    /// Returns the available analyzer types.
    ///
    /// This function returns a vector of all available analyzer types. This is needed for
    /// the backend / gui to determine which analyzers are available.
    #[must_use]
    pub fn analyzer_types() -> Vec<Self> {
        Self::iter().collect()
    }
}
impl Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Energy => "Energy",
            Self::RayTrace(_) => "RayTracing",
            Self::GhostFocus(_) => "GhostFocus",
        };
        write!(f, "{msg}")
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn display() {
        assert_eq!(format!("{}", AnalyzerType::Energy), "Energy");
        assert_eq!(
            format!("{}", AnalyzerType::RayTrace(RayTraceConfig::default())),
            "RayTracing"
        );
        assert_eq!(
            format!("{}", AnalyzerType::GhostFocus(GhostFocusConfig::default())),
            "GhostFocus"
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

    /// Generate an analysis report for this [`NodeGroup`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport>;
}
