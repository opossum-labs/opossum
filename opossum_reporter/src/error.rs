//! Error types for the opossum_reporter crate.

use std::fmt;

/// Result type shorthand for opossum_reporter operations.
pub type ReporterResult<T> = Result<T, ReporterError>;

/// Errors that can occur during report generation, plotting, or graph export.
#[derive(Debug)]
pub enum ReporterError {
    /// Errors originating from `opossum_core`.
    Core(opossum_core::error::OpossumError),
    /// Standard I/O errors.
    Io(std::io::Error),
    /// Template compilation or rendering errors.
    Template(String),
    /// Plotting or image generation errors.
    Plot(String),
    /// Graphviz DOT syntax or SVG generation errors.
    Graphviz(String),
    /// HTML report generation errors.
    Html(String),
    /// Invalid report structure (e.g., missing scenery).
    InvalidReport(String),
    /// Other reporting errors.
    Other(String),
}

impl fmt::Display for ReporterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(e) => write!(f, "Core error: {e}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Template(msg) => write!(f, "Template error: {msg}"),
            Self::Plot(msg) => write!(f, "Plot error: {msg}"),
            Self::Graphviz(msg) => write!(f, "Graphviz error: {msg}"),
            Self::Html(msg) => write!(f, "HTML report error: {msg}"),
            Self::InvalidReport(msg) => write!(f, "Invalid report: {msg}"),
            Self::Other(msg) => write!(f, "Reporter error: {msg}"),
        }
    }
}

impl std::error::Error for ReporterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Core(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<opossum_core::error::OpossumError> for ReporterError {
    fn from(err: opossum_core::error::OpossumError) -> Self {
        Self::Core(err)
    }
}

impl From<std::io::Error> for ReporterError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
