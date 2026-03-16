//! Analyzer performing a ghost focus analysis using ray tracing
#![warn(missing_docs)]

mod analyzer;
mod config;
mod history;

pub use analyzer::{AnalysisGhostFocus, GhostFocusAnalyzer};
pub use config::GhostFocusConfig;
pub use history::GhostFocusHistory;

use log::warn;

use super::{AnalyzerRegistration, AnalyzerType};

inventory::submit! {
    AnalyzerRegistration::new(
        || AnalyzerType::GhostFocus(GhostFocusConfig::default()),
        |at| if let AnalyzerType::GhostFocus(config) = at { Some(Box::new(GhostFocusAnalyzer::new(config.clone()))) } else { None }
    )
}
