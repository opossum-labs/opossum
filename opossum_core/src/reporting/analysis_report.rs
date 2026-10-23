#![warn(missing_docs)]
//! Module handling analysis reports, serialization, and report generation.

use super::{node_report::NodeReport, report_note::ReportNote};
use crate::{
    error::{OpmResult, OpossumError},
    get_version,
    nodes::NodeGroup,
};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Debug, Clone, Deserialize)]
/// Structure for storing data being integrated in an analysis report.
pub struct AnalysisReport {
    opossum_version: String,
    analysis_timestamp: DateTime<Local>,
    analysis_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    analyzer_name: Option<String>,
    scenery: Option<NodeGroup>,
    node_reports: Vec<NodeReport>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    notes: Vec<ReportNote>,
}

impl Default for AnalysisReport {
    fn default() -> Self {
        Self {
            opossum_version: get_version(),
            analysis_timestamp: Local::now(),
            analysis_type: String::default(),
            analyzer_name: None,
            scenery: None,
            node_reports: Vec::default(),
            notes: Vec::default(),
        }
    }
}

impl AnalysisReport {
    /// Creates a new [`AnalysisReport`] with the current version and local timestamp.
    #[must_use]
    pub fn new(analysis_type: impl Into<String>) -> Self {
        Self {
            opossum_version: get_version(),
            analysis_timestamp: Local::now(),
            analysis_type: analysis_type.into(),
            analyzer_name: None,
            scenery: None,
            node_reports: Vec::default(),
            notes: Vec::default(),
        }
    }
    /// Returns the user-assigned analyzer name of this [`AnalysisReport`], if set.
    #[must_use]
    pub fn analyzer_name(&self) -> Option<&str> {
        self.analyzer_name.as_deref()
    }

    /// Sets the analyzer name of this [`AnalysisReport`].
    ///
    /// Empty strings and whitespace-only strings are normalized to `None`.
    pub fn set_analyzer_name(&mut self, analyzer_name: Option<String>) {
        self.analyzer_name = analyzer_name.filter(|name| !name.trim().is_empty());
    }

    /// Sets the scenery [`NodeGroup`] of this [`AnalysisReport`].
    ///
    /// Moves the provided scenery into the report without cloning.
    pub fn set_scenery(&mut self, scenery: NodeGroup) {
        self.scenery = Some(scenery);
    }

    /// Adds a [`NodeReport`] to this [`AnalysisReport`].
    ///
    /// After analysis of a [`NodeGroup`], each node can generate a [`NodeReport`] using the
    /// [`report`](crate::analyzers::Analyzer::report) trait function. While assembling a report this
    /// function adds the node data to it.
    pub fn add_node_report(&mut self, report: NodeReport) {
        self.node_reports.push(report);
    }

    /// Adds a note to this [`AnalysisReport`].
    pub fn add_note(&mut self, note: ReportNote) {
        self.notes.push(note);
    }

    /// Returns a reference to the notes of this [`AnalysisReport`].
    #[must_use]
    pub fn notes(&self) -> &[ReportNote] {
        &self.notes
    }

    /// Returns a reference to the scenery of this [`AnalysisReport`], if present.
    #[must_use]
    pub const fn scenery(&self) -> Option<&NodeGroup> {
        self.scenery.as_ref()
    }

    /// Serializes this [`AnalysisReport`] to a compact RON string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization to RON fails.
    pub fn to_file_string(&self) -> OpmResult<String> {
        ron::to_string(&self)
            .map_err(|e| OpossumError::Analysis(format!("Error serializing AnalysisReport: {e}")))
    }

    /// Deserializes an [`AnalysisReport`] from a RON formatted string.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails.
    pub fn from_ron_str(content: &str) -> OpmResult<Self> {
        ron::from_str(content)
            .map_err(|e| OpossumError::Analysis(format!("Error deserializing AnalysisReport: {e}")))
    }

    /// Loads an [`AnalysisReport`] from the specified RON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened, read, or deserialized.
    pub fn load_from_ron_file(path: &Path) -> OpmResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            OpossumError::Analysis(format!("Failed to read RON file '{}': {e}", path.display()))
        })?;
        Self::from_ron_str(&content)
    }

    /// Saves the compact RON representation to the given directory.
    ///
    /// # Errors
    ///
    /// Returns an error if file creation or serialization fails.
    pub fn save_ron(&self, report_directory: &Path, report_number: usize) -> OpmResult<()> {
        let ron_path = report_directory.join(format!("report_{report_number}.ron"));
        let content = self.to_file_string()?;
        std::fs::write(&ron_path, content).map_err(|e| {
            OpossumError::Analysis(format!(
                "Writing compact RON file failed at '{}': {e}",
                ron_path.display()
            ))
        })?;

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

    /// Returns a mutable slice of node reports of this [`AnalysisReport`].
    pub fn node_reports_mut(&mut self) -> &mut [NodeReport] {
        &mut self.node_reports
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{properties::Properties, reporting::report_note::ReportLevel};

    #[test]
    fn new_and_default() {
        let report = AnalysisReport::new("Raytracing");
        assert!(report.scenery().is_none());
        assert_eq!(report.opossum_version(), get_version());
        assert_eq!(report.analysis_type(), "Raytracing");
        assert!(report.node_reports().is_empty());
        assert!(report.notes().is_empty());

        let default_report = AnalysisReport::default();
        assert_eq!(default_report.opossum_version(), get_version());
        assert_eq!(default_report.analysis_type(), "");
    }
    #[test]
    fn set_analysis_type() {
        let mut report = AnalysisReport::default();
        report.set_analysis_type("Wavefront Propagation");
        assert_eq!(report.analysis_type(), "Wavefront Propagation");
    }

    #[test]
    fn set_and_get_scenery() {
        let mut report = AnalysisReport::default();
        assert!(report.scenery().is_none());

        report.set_scenery(NodeGroup::default());
        assert!(report.scenery().is_some());
    }

    #[test]
    fn add_node_report_and_notes() {
        let mut report = AnalysisReport::default();
        let node_rep = NodeReport::new(
            "test detector",
            "detector name",
            "123",
            Properties::default(),
        );
        report.add_node_report(node_rep);
        assert_eq!(report.node_reports().len(), 1);

        let note = ReportNote::new(ReportLevel::Info, "Test description text");
        report.add_note(note);
        assert_eq!(report.notes().len(), 1);
    }

    #[test]
    fn compact_ron_roundtrip() -> OpmResult<()> {
        let mut report = AnalysisReport::new("Diagnostic Check");
        report.set_analyzer_name(Some("Main Beam Analyzer".to_string()));
        report.set_scenery(NodeGroup::default());
        report.add_note(ReportNote::new(ReportLevel::Info, "Detail"));

        let serialized = report.to_file_string()?;
        // Compact format must not contain formatted newlines within fields
        assert!(!serialized.contains("\n  "));

        let deserialized = AnalysisReport::from_ron_str(&serialized)?;
        assert_eq!(report.analysis_type(), deserialized.analysis_type());
        assert_eq!(report.analyzer_name(), deserialized.analyzer_name());
        assert_eq!(report.opossum_version(), deserialized.opossum_version());
        assert_eq!(report.notes().len(), deserialized.notes().len());

        Ok(())
    }

    #[test]
    fn save_ron_buffered() -> OpmResult<()> {
        let report = AnalysisReport::new("Unit Test Run");
        let temp_dir = tempfile::tempdir()
            .map_err(|e| OpossumError::Other(format!("Failed to create temp dir: {e}")))?;

        let invalid_path = temp_dir.path().join("missing").join("subfolder");
        assert!(report.save_ron(&invalid_path, 0).is_err());

        report.save_ron(temp_dir.path(), 42)?;
        let target_file = temp_dir.path().join("report_42.ron");
        assert!(target_file.is_file());

        let loaded_report = AnalysisReport::load_from_ron_file(&target_file)?;
        assert_eq!(loaded_report.analysis_type(), "Unit Test Run");

        Ok(())
    }

    #[test]
    fn analyzer_name_normalization() {
        let mut report = AnalysisReport::default();
        assert_eq!(report.analyzer_name(), None);

        report.set_analyzer_name(Some("Main Raytracer".to_string()));
        assert_eq!(report.analyzer_name(), Some("Main Raytracer"));

        report.set_analyzer_name(Some("   ".to_string()));
        assert_eq!(report.analyzer_name(), None);

        report.set_analyzer_name(Some(String::new()));
        assert_eq!(report.analyzer_name(), None);

        report.set_analyzer_name(None);
        assert_eq!(report.analyzer_name(), None);
    }
}
