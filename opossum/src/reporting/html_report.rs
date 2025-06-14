//! Module for generating html reports from analysis results.
use serde::Serialize;
use std::{fs, path::Path};
use tinytemplate::TinyTemplate;

use crate::error::{OpmResult, OpossumError};

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
    /// Generate an html report from this [`HtmlReport`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - underlying templates could not be compiled.
    ///   - the base file name could not be determined.
    ///   - the conversion
    pub fn generate_html(&self, path: &Path) -> OpmResult<()> {
        let mut tt = TinyTemplate::new();
        tt.add_template("report", HTML_REPORT)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        tt.add_template("node_report", HTML_NODE_REPORT)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        let rendered = tt
            .render("report", &self)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        fs::write(path, rendered).map_err(|e| OpossumError::Other(e.to_string()))?;
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
