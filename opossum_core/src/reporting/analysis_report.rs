#![warn(missing_docs)]
//! Module handling analysis reports and converting them to HTML.

use std::{fs::File, io::Write, path::Path};

use super::{html_report::HtmlReport, node_report::NodeReport};
use crate::{
    error::{OpmResult, OpossumError},
    get_version,
    nodes::NodeGroup,
};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug, Clone, Deserialize)]
/// Structure for storing data being integrated in an analysis report.
pub struct AnalysisReport {
    opossum_version: String,
    analysis_timestamp: DateTime<Local>,
    analysis_type: String,
    scenery: Option<NodeGroup>,
    node_reports: Vec<NodeReport>,
}
impl Default for AnalysisReport {
    fn default() -> Self {
        Self {
            opossum_version: get_version(),
            analysis_timestamp: Local::now(),
            analysis_type: String::default(),
            scenery: None,
            node_reports: Vec::default(),
        }
    }
}
impl AnalysisReport {
    /// Creates a new [`AnalysisReport`].
    #[must_use]
    pub fn new(opossum_version: String, analysis_timestamp: DateTime<Local>) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            analysis_type: String::default(),
            scenery: None,
            node_reports: Vec::default(),
        }
    }
    /// Add an [`NodeGroup`] to this [`AnalysisReport`].
    ///
    /// This function is called internally by the top level [`NodeGroup`] for adding itself to the report.
    pub fn add_scenery(&mut self, scenery: &NodeGroup) {
        self.scenery = Some(scenery.clone());
    }
    /// Add a [`NodeReport`] to this [`AnalysisReport`].
    ///
    /// After analysis of a [`NodeGroup`], each node can generate a [`NodeReport`] using the
    /// [`report`](crate::analyzers::Analyzer::report) trait function. While assembling a report this
    /// function adds the node data to it. This is mostly interesting for detector nodes which deliver
    /// their particular analysis result.
    pub fn add_node_report(&mut self, report: NodeReport) {
        self.node_reports.push(report);
    }
    /// Returns the scenery of this [`AnalysisReport`].
    #[must_use]
    pub fn scenery(&self) -> Option<NodeGroup> {
        self.scenery.clone()
    }
    /// Serialize this [`AnalysisReport`] to a file string.
    ///
    /// # Errors
    ///
    /// This function will return an error if the serialization of the [`AnalysisReport`] fails.
    pub fn to_file_string(&self) -> OpmResult<String> {
        ron::ser::to_string_pretty(&self, ron::ser::PrettyConfig::new().new_line("\n"))
            .map_err(|e| OpossumError::Other(format!("Error serializing AnalysisReport: {e}")))
    }
    /// Saves the complete report to the specified directory.
    ///
    /// This creates a RON file for the report data, an HTML representation,
    /// and exports all associated data files (e.g., plots, images).
    ///
    /// # Errors
    ///
    /// Returns an error if any file I/O or data export operation fails.
    pub fn save(&self, report_directory: &Path, report_number: usize) -> OpmResult<()> {
        // 1. Save the RON file
        let ron_path = report_directory.join(format!("report_{report_number}.ron"));
        let mut ron_file = File::create(ron_path)
            .map_err(|e| OpossumError::Other(format!("RON file creation failed: {e}")))?;
        write!(ron_file, "{}", self.to_file_string()?)
            .map_err(|e| OpossumError::Other(format!("writing RON file failed: {e}")))?;

        // 2. Save the HTML report (including data files)
        let html_report = HtmlReport::from_analysis_report(self)?;
        html_report.generate_report_files(report_directory, self, report_number)?;
        Ok(())
    }
    /// Sets the analysis type of this [`AnalysisReport`].
    ///
    /// This information is used i.e. in the [`HtmlReport`].
    pub fn set_analysis_type(&mut self, analysis_type: &str) {
        analysis_type.clone_into(&mut self.analysis_type);
    }
    /// Returns a reference to the opossum version of this [`AnalysisReport`].
    #[must_use]
    pub fn opossum_version(&self) -> &str {
        &self.opossum_version
    }
    /// Returns the analysis timestamp of this [`AnalysisReport`].
    #[must_use]
    pub const fn analysis_timestamp(&self) -> DateTime<Local> {
        self.analysis_timestamp
    }
    /// Returns a reference to the analysis type of this [`AnalysisReport`].
    #[must_use]
    pub fn analysis_type(&self) -> &str {
        &self.analysis_type
    }
    /// Returns a reference to the node reports of this [`AnalysisReport`].
    #[must_use]
    pub fn node_reports(&self) -> &[NodeReport] {
        &self.node_reports
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{opm_document::OpmDocument, properties::Properties};
    #[test]
    fn new() {
        let timestamp = Local::now();
        let report = AnalysisReport::new(String::from("test"), timestamp);
        assert!(report.scenery.is_none());
        assert_eq!(report.opossum_version, "test");
        assert!(report.node_reports.is_empty());
        assert_eq!(report.analysis_timestamp, timestamp);
    }
    #[test]
    fn default() {
        let report = AnalysisReport::default();
        assert!(report.scenery.is_none());
        assert_eq!(report.opossum_version, get_version());
        assert!(report.node_reports.is_empty());
    }
    #[test]
    fn set_analysis_type() {
        let timestamp = Local::now();
        let mut report = AnalysisReport::new(String::from("test"), timestamp);
        report.set_analysis_type("my type");
        assert_eq!(report.analysis_type, "my type");
    }
    #[test]
    fn add_scenery() {
        let mut report = AnalysisReport::new(String::from("test"), DateTime::default());
        report.add_scenery(&NodeGroup::default());
        assert!(report.scenery.is_some());
    }
    #[test]
    fn add_node_report() {
        let mut report = AnalysisReport::new(String::from("test"), DateTime::default());
        report.add_node_report(NodeReport::new(
            "test detector",
            "detector name",
            "123",
            Properties::default(),
        ));
        assert_eq!(report.node_reports.len(), 1);
    }
    #[test]
    fn save() {
        let mut document =
            OpmDocument::from_file(&Path::new("./files_for_testing/opm/opticscenery.opm")).unwrap();
        let reports = document.analyze().unwrap();
        assert!(
            reports[0]
                .save(&Path::new("./files_for_testing/report/_not_valid/"), 0)
                .is_err()
        );
    }
}
