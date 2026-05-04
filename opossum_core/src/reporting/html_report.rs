//! Module for generating html reports from analysis results.
use log::info;
use serde::Serialize;
use std::{fs, path::Path};
use tinytemplate::TinyTemplate;

use crate::{
    core_optics::OpticNode,
    error::{OpmResult, OpossumError},
    reporting::{
        analysis_report::AnalysisReport,
        node_report::NodeReport,
        report_note::{ReportLevel, ReportNote},
    },
    utils::file_utils::sanitize_filename,
};

#[derive(Serialize)]
pub struct HtmlReportNote {
    pub message: String,
    pub css_class: String,
}

impl From<ReportNote> for HtmlReportNote {
    fn from(note: ReportNote) -> Self {
        let css_class = match note.level {
            ReportLevel::Info => "alert-info",
            ReportLevel::Warning => "alert-warning",
            ReportLevel::Error => "alert-danger",
        };
        Self {
            message: note.message,
            css_class: css_class.to_string(),
        }
    }
}

static HTML_REPORT: &str = include_str!("../html/html_report.html");
static HTML_NODE_REPORT: &str = include_str!("../html/node_report.html");

#[derive(Serialize)]
pub struct HtmlReport {
    opossum_version: String,
    analysis_timestamp: String,
    analysis_type: String,
    description: String,
    node_reports: Vec<HtmlNodeReport>,
    notes: Vec<HtmlReportNote>,
}
impl HtmlReport {
    #[must_use]
    pub const fn new(
        opossum_version: String,
        analysis_timestamp: String,
        analysis_type: String,
        description: String,
        node_reports: Vec<HtmlNodeReport>,
        notes: Vec<HtmlReportNote>,
    ) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            analysis_type,
            description,
            node_reports,
            notes,
        }
    }
    /// Creates a new [`HtmlReport`] from an [`AnalysisReport`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the provided [`AnalysisReport`] has an empty scenery.
    pub fn from_analysis_report(report: &AnalysisReport, report_number: usize) -> OpmResult<Self> {
        let Some(scenery) = &report.scenery() else {
            return Err(OpossumError::Other("no scenery found".into()));
        };
        let html_node_reports: Vec<HtmlNodeReport> = report
            .node_reports()
            .iter()
            .map(|node_report| HtmlNodeReport::from_node_report(node_report, report_number))
            .collect();

        Ok(Self::new(
            report.opossum_version().to_string(),
            report
                .analysis_timestamp()
                .format("%Y/%m/%d %H:%M")
                .to_string(),
            report.analysis_type().to_string(),
            scenery.node_attr().name(),
            html_node_reports,
            report
                .notes()
                .iter()
                .map(|n| HtmlReportNote::from(n.clone()))
                .collect(),
        ))
    }
    /// Generate a complete HTML report, including the main HTML file
    /// and all associated data files.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - the data directory `data` cannot be created.
    /// - the main HTML file could not be written.
    /// - the export of data files fails.
    pub fn generate_report_files(
        &self,
        report_path: &Path,
        analysis_report: &AnalysisReport,
        report_number: usize,
    ) -> OpmResult<()> {
        // 1. Export associated data first
        let data_dir = report_path.join(format!("data_{report_number}"));
        fs::create_dir_all(&data_dir).map_err(|e| {
            OpossumError::Other(format!("Error creating data dir for html report: {e}"))
        })?;
        for node_report in analysis_report.node_reports() {
            node_report.export(&data_dir)?;
        }

        // 2. Render and write the main HTML file
        let mut tt = TinyTemplate::new();
        tt.add_template("report", HTML_REPORT).map_err(|e| {
            OpossumError::Other(format!("Error adding html `report` template: {e}"))
        })?;
        tt.add_template("node_report", HTML_NODE_REPORT)
            .map_err(|e| {
                OpossumError::Other(format!("Error adding html `node_report` template: {e}"))
            })?;
        let rendered = tt.render("report", &self).map_err(|e| {
            OpossumError::Other(format!("Error rendering html `report` template: {e}"))
        })?;
        let html_path = report_path.join(format!("report_{report_number}.html"));
        fs::write(&html_path, rendered)
            .map_err(|e| OpossumError::Other(format!("Error writing html file: {e}")))?;
        info!("Write html report to {}", html_path.display());
        Ok(())
    }
}
/// Structure for storing a node report during html conversion.
#[derive(Serialize)]
pub struct HtmlNodeReport {
    /// node name
    pub node_name: String,
    /// node type
    pub node_type: String,
    /// properties of the node
    pub props: Vec<HtmlProperty>,
    /// uuid of the node (needed for constructing filenames)
    pub uuid: String,
    /// show or hide item in report by default
    pub show_item: bool,
    /// notes regarding the node
    pub notes: Vec<HtmlReportNote>,
}
impl HtmlNodeReport {
    /// Create this [`HtmlNodeReport`] from an [`NodeReport`].
    #[must_use]
    pub fn from_node_report(node_report: &NodeReport, report_number: usize) -> Self {
        Self {
            node_name: sanitize_filename(node_report.name()),
            node_type: node_report.node_type().to_string(),
            props: node_report
                .properties()
                .html_props(node_report.uuid(), report_number),
            uuid: node_report.uuid().to_string(),
            show_item: node_report.show_item(),
            notes: node_report
                .notes()
                .iter()
                .map(|n| HtmlReportNote::from(n.clone()))
                .collect(),
        }
    }
}
#[derive(Serialize)]
pub struct HtmlProperty {
    pub name: String,
    pub description: String,
    pub prop_value: String,
}

#[cfg(test)]
mod test {
    use crate::{
        error::OpmResult,
        properties::Properties,
        reporting::{html_report::HtmlNodeReport, node_report::NodeReport},
    };
    #[test]
    fn from_node_report() -> OpmResult<()> {
        let mut properties = Properties::default();
        properties.create("test1", "desc1", 1.0.into())?;
        properties.create("test2", "desc2", "test".into())?;
        let report = NodeReport::new("test detector", "detector name", "123", properties);
        let html_report = HtmlNodeReport::from_node_report(&report, 0);
        assert_eq!(html_report.node_name, "detector name");
        assert_eq!(html_report.node_type, "test detector");
        assert_eq!(html_report.uuid, "123");
        assert_eq!(html_report.show_item, false);
        assert!(html_report.notes.is_empty());
        let html_props = html_report.props;

        assert_eq!(html_props[0].name, "test1");
        assert_eq!(html_props[0].description, "desc1");
        assert_eq!(html_props[0].prop_value, "1.000000");
        Ok(())
    }
}
