#![warn(missing_docs)]
//! Module handling analysis reports and converting them to HTML.

use std::{fs, path::Path};

use super::{
    html_report::{HtmlNodeReport, HtmlReport},
    node_report::NodeReport,
};
use crate::{
    error::{OpmResult, OpossumError},
    get_version,
    nodes::NodeGroup,
    optic_node::OpticNode,
};
use chrono::{DateTime, Local};
use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
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
    /// Export data of each [`NodeReport`] of this [`AnalysisReport`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the individual `export_data` function of the individual
    /// nodes fails.
    pub fn export_data(&self, report_path: &Path) -> OpmResult<()> {
        let report_path = report_path.join(Path::new("data"));
        if !report_path.exists() {
            return Err(OpossumError::Other("report path does not exist".into()));
        }
        let md = fs::metadata(&report_path).map_err(|e| {
            OpossumError::Other(format!("could not check directory permissions: {e}"))
        })?;
        let permissions = md.permissions();
        if permissions.readonly() {
            return Err(OpossumError::Other(
                "report path dow not have write permissions".into(),
            ));
        }
        for node_report in &self.node_reports {
            node_report.export_data(&report_path, "")?;
        }
        Ok(())
    }
    /// Generate an [`HtmlReport`] from this [`AnalysisReport`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the report has no scenery set.
    pub fn to_html_report(&self) -> OpmResult<HtmlReport> {
        let Some(scenery) = &self.scenery else {
            return Err(OpossumError::Other("no scenery found".into()));
        };
        let html_node_reports: Vec<HtmlNodeReport> = self
            .node_reports
            .iter()
            .map(|r| r.to_html_node_report(""))
            .collect();
        Ok(HtmlReport::new(
            self.opossum_version.clone(),
            self.analysis_timestamp.format("%Y/%m/%d %H:%M").to_string(),
            self.analysis_type.clone(),
            scenery.node_attr().name(),
            html_node_reports,
        ))
    }
    /// Sets the analysis type of this [`AnalysisReport`].
    ///
    /// This information is used i.e. in the [`HtmlReport`].
    pub fn set_analysis_type(&mut self, analysis_type: &str) {
        analysis_type.clone_into(&mut self.analysis_type);
    }
}

#[cfg(test)]
mod test {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::properties::Properties;
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
    fn to_html_report() {
        let mut report = AnalysisReport::default();
        assert!(report.to_html_report().is_err());
        report.add_scenery(&NodeGroup::default());
        assert!(report.to_html_report().is_ok());
        // further details to be checked in html_reports module (private fields...)
    }
    #[test]
    fn export_data() {
        let report = AnalysisReport::default();
        assert!(report.export_data(Path::new("")).is_err());
        let tmp_dir = TempDir::new().unwrap();
        fs::create_dir(tmp_dir.path().join("data")).unwrap();
        assert!(report.export_data(tmp_dir.path()).is_ok());
    }
}
