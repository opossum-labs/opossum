#![warn(missing_docs)]
//! Module for generating analysis reports in PDF format.

use crate::{
    analyzer::AnalyzerType,
    error::{OpmResult, OpossumError},
    properties::{property::HtmlProperty, Properties, Proptype},
    OpticScenery,
};
use chrono::{DateTime, Local};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tinytemplate::TinyTemplate;

static HTML_REPORT: &str = include_str!("html/html_report.html");
static HTML_DIAGRAM: &str = include_str!("html/diagram.html");
static HTML_NODE_REPORT: &str = include_str!("html/node_report.html");

#[derive(Serialize)]
struct HtmlScenery {
    description: String,
    url: String,
}
/// Structure for storing a (detector) node report during html conversion.
#[derive(Serialize)]
pub struct HtmlNodeReport {
    /// node name
    pub node: String,
    /// node type
    pub node_type: String,
    /// properties of the node
    pub props: Vec<HtmlProperty>,
}
#[derive(Serialize)]
struct HtmlReport {
    opossum_version: String,
    analysis_timestamp: String,
    scenery: HtmlScenery,
    node_reports: Vec<HtmlNodeReport>,
}
#[derive(Serialize, Debug, Clone)]
/// Structure for storing data being integrated in an analysis report.
pub struct AnalysisReport {
    opossum_version: String,
    analysis_timestamp: DateTime<Local>,
    scenery: Option<OpticScenery>,
    node_reports: Vec<NodeReport>,
}
impl AnalysisReport {
    /// Creates a new [`AnalysisReport`].
    #[must_use]
    pub fn new(opossum_version: String, analysis_timestamp: DateTime<Local>) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            scenery: None,
            node_reports: Vec::default(),
        }
    }
    /// Add an [`OpticScenery`] to this [`AnalysisReport`].
    ///
    /// This function is called internally [`OpticScenery`] for adding itself to the report.
    pub fn add_scenery(&mut self, scenery: &OpticScenery) {
        self.scenery = Some(scenery.clone());
    }
    /// Add an (detector) [`NodeReport`] to this [`AnalysisReport`].
    ///
    /// After analysis of an [`OpticScenery`], ech node can generate a [`NodeReport`] using the `Optical::report` trait function.
    /// While assembling a report this function adds the node data to it. This is mostly interesting for detector nodes which deliver
    /// their particular analysis result.
    pub fn add_detector(&mut self, report: NodeReport) {
        self.node_reports.push(report);
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
/// Structure for storing (detector-)node specific data to be integrated in the [`AnalysisReport`].
pub struct NodeReport {
    detector_type: String,
    name: String,
    properties: Properties,
}
impl NodeReport {
    /// Creates a new [`NodeReport`].
    #[must_use]
    pub fn new(detector_type: &str, name: &str, properties: Properties) -> Self {
        Self {
            detector_type: detector_type.to_owned(),
            name: name.to_owned(),
            properties,
        }
    }
    /// Returns a reference to the detector type of this [`NodeReport`].
    #[must_use]
    pub fn detector_type(&self) -> &str {
        self.detector_type.as_ref()
    }
    /// Returns a reference to the name of this [`NodeReport`].
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
    /// Returns a reference to the properties of this [`NodeReport`].
    #[must_use]
    pub const fn properties(&self) -> &Properties {
        &self.properties
    }
}

impl From<NodeReport> for Proptype {
    fn from(value: NodeReport) -> Self {
        Self::NodeReport(value)
    }
}
/// Report generator
///
/// This report generator delivers a PDF file containing the analysis report based on the provided [`AnalysisReport`].
#[derive(Clone)]
pub struct ReportGenerator {
    base_file_name: PathBuf,
    report: AnalysisReport,
}

impl ReportGenerator {
    /// Creates a new [`ReportGenerator`].
    #[must_use]
    pub fn new(report: AnalysisReport, base_file_name: &Path) -> Self {
        Self {
            report,
            base_file_name: PathBuf::from(base_file_name),
        }
    }
    /// Generate an html report.
    ///
    /// # Panics
    ///
    /// Panics if .
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - underlying templates could not be compiled.
    ///   - the base file name could not be determined.
    pub fn generate_html(&self, path: &Path, _analyzer: &AnalyzerType) -> OpmResult<()> {
        info!("Write html report to {}", path.display());
        let mut tt = TinyTemplate::new();
        tt.add_template("report", HTML_REPORT)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        tt.add_template("diagram", HTML_DIAGRAM)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        tt.add_template("node_report", HTML_NODE_REPORT)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        let Some(scenery) = &self.report.scenery else {
            return Err(OpossumError::Other("no scenery found".into()));
        };
        let mut diagram_path = self.base_file_name.clone();
        diagram_path.set_extension("svg");
        let diagram_url = diagram_path
            .file_name()
            .ok_or_else(|| OpossumError::Other("could not determine base file name".into()))?
            .to_os_string()
            .into_string()
            .unwrap();
        let html_scenery = HtmlScenery {
            description: scenery.description().into(),
            url: diagram_url,
        };
        let mut node_reports: Vec<HtmlNodeReport> = Vec::new();
        for report in &self.report.node_reports {
            let html_node_report = HtmlNodeReport {
                node: report.name().into(),
                node_type: report.detector_type().into(),
                props: report.properties().html_props(report.name()),
            };
            node_reports.push(html_node_report);
        }
        let html_report = HtmlReport {
            opossum_version: self.report.opossum_version.clone(),
            analysis_timestamp: self
                .report
                .analysis_timestamp
                .format("%Y/%m/%d %H:%M")
                .to_string(),
            scenery: html_scenery,
            node_reports,
        };
        let rendered = tt
            .render("report", &html_report)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        fs::write(path, rendered).map_err(|e| OpossumError::Other(e.to_string()))?;
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn analysis_report_new() {
        let timestamp = Local::now();
        let report = AnalysisReport::new(String::from("test"), timestamp);
        assert!(report.scenery.is_none());
        assert_eq!(report.opossum_version, "test");
        assert!(report.node_reports.is_empty());
        assert_eq!(report.analysis_timestamp, timestamp);
    }
    #[test]
    fn analysis_report_add_scenery() {
        let mut report = AnalysisReport::new(String::from("test"), DateTime::default());
        report.add_scenery(&OpticScenery::default());
        assert!(report.scenery.is_some());
    }
    #[test]
    fn analysis_report_add_detector() {
        let mut report = AnalysisReport::new(String::from("test"), DateTime::default());
        report.add_detector(NodeReport::new(
            "test detector",
            "detector name",
            Properties::default(),
        ));
        assert_eq!(report.node_reports.len(), 1);
    }
    #[test]
    fn node_report_new() {
        let report = NodeReport::new("test detector", "detector name", Properties::default());
        assert_eq!(report.detector_type, "test detector");
        assert_eq!(report.name, "detector name");
    }
}
