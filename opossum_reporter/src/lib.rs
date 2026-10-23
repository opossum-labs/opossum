//! Report generation, visualization, and plotting library for OPOSSUM.
//!
//! This crate takes the raw `AnalysisReport` produced by `opossum_core`
//! and generates:
//! - HTML reports with embedded or linked images,
//! - 2D/3D plots and SVG/PNG diagrams (`plotters`),
//! - Graphviz `.dot` and `.svg` graphs of the optical layout.

pub mod error;
pub mod graphviz;
pub mod html_report;
pub mod plotting;

pub use error::{ReporterError, ReporterResult};
pub use graphviz::create_dot_file;
pub use html_report::{HtmlNodeReport, HtmlProperty, HtmlReport, HtmlReportNote};

use opossum_core::reporting::AnalysisReport;
use std::path::Path;

/// Saves the complete report to the specified directory.
///
/// This creates a compact RON file for the report data, an HTML representation,
/// and exports all associated data files (e.g., plots, images).
///
/// # Errors
///
/// Returns an error if saving the RON file, creating the HTML report,
/// or exporting plot data fails.
pub fn save_report(
    report: &AnalysisReport,
    report_directory: &Path,
    report_number: usize,
) -> ReporterResult<()> {
    report.save_ron(report_directory, report_number)?;
    let html_report = HtmlReport::from_analysis_report(report, report_number)?;
    html_report.generate_report_files(report_directory, report, report_number)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::opm_document::OpmDocument;
    use std::fs;

    #[test]
    fn test_save_report() -> ReporterResult<()> {
        let mut document =
            OpmDocument::from_file(Path::new("./files_for_testing/opm/opticscenery.opm"))?;
        let reports = document.analyze()?;
        let report = &reports[0];

        let temp_dir = tempfile::tempdir()?;

        let non_existent_path = temp_dir.path().join("missing_folder").join("nested");
        assert!(
            save_report(report, &non_existent_path, 0).is_err(),
            "Saving to an invalid directory path must return an error"
        );

        let valid_target = temp_dir.path();
        save_report(report, valid_target, 0)?;

        let ron_file = valid_target.join("report_0.ron");
        let html_file = valid_target.join("report_0.html");
        let data_dir = valid_target.join("data_0");

        assert!(ron_file.is_file(), "Expected 'report_0.ron' to be created");
        assert!(
            html_file.is_file(),
            "Expected 'report_0.html' to be created"
        );
        assert!(
            data_dir.is_dir(),
            "Expected 'data_0' directory to be created"
        );

        let ron_len = fs::metadata(&ron_file)?.len();
        assert!(ron_len > 0, "Generated 'report_0.ron' file is empty");

        Ok(())
    }
}
