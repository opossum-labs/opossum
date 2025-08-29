//! Module for generating html reports from analysis results.
use log::info;
use serde::Serialize;
use std::{fs, path::Path};
use tinytemplate::TinyTemplate;

use crate::{
    error::{OpmResult, OpossumError},
    optic_node::OpticNode,
    reporting::analysis_report::AnalysisReport,
};

static HTML_REPORT: &str = include_str!("../html/html_report.html");
static HTML_NODE_REPORT: &str = include_str!("../html/node_report.html");

#[derive(Serialize)]
pub struct HtmlReport {
    opossum_version: String,
    analysis_timestamp: String,
    analysis_type: String,
    description: String,
    node_reports: Vec<HtmlNodeReport>,
}
impl HtmlReport {
    #[must_use]
    pub const fn new(
        opossum_version: String,
        analysis_timestamp: String,
        analysis_type: String,
        description: String,
        node_reports: Vec<HtmlNodeReport>,
    ) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            analysis_type,
            description,
            node_reports,
        }
    }
    /// Creates a new [`HtmlReport`] from an [`AnalysisReport`].
    ///
    /// # Errors
    ///
    /// This function returns an error if the provided [`AnalysisReport`] has an empty scenery.
    pub fn from_analysis_report(report: &AnalysisReport) -> OpmResult<Self> {
        let Some(scenery) = &report.scenery() else {
            return Err(OpossumError::Other("no scenery found".into()));
        };
        let html_node_reports: Vec<HtmlNodeReport> = report
            .node_reports()
            .iter()
            .map(|r| r.to_html_node_report(""))
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
        let data_dir = report_path.join("data");
        fs::create_dir_all(&data_dir).map_err(|e| {
            OpossumError::Other(format!("Error creating data dir for html report: {e}"))
        })?;
        for node_report in analysis_report.node_reports() {
            node_report.properties().export_data(
                &data_dir,
                &format!("_{}_{}", &node_report.name(), &node_report.uuid()),
            )?;
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
}

#[derive(Serialize)]
pub struct HtmlProperty {
    pub name: String,
    pub description: String,
    pub prop_value: String,
}

#[cfg(test)]
mod test {
    // #[test]
    // fn from_analysis_report() {
    // }
    // #[test]
    // fn generate_report_files() {
    // }
}
