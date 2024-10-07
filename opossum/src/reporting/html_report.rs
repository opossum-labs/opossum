use std::{fs, path::Path};

use log::info;
use serde::Serialize;
use tinytemplate::TinyTemplate;

use crate::{
    error::{OpmResult, OpossumError},
    properties::property::HtmlProperty,
};

static HTML_REPORT: &str = include_str!("../html/html_report.html");
static HTML_NODE_REPORT: &str = include_str!("../html/node_report.html");

#[derive(Serialize)]
pub struct HtmlReport {
    opossum_version: String,
    analysis_timestamp: String,
    description: String,
    node_reports: Vec<HtmlNodeReport>,
}
impl HtmlReport {
    pub fn new(
        opossum_version: String,
        analysis_timestamp: String,
        description: String,
        node_reports: Vec<HtmlNodeReport>,
    ) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            description,
            node_reports,
        }
    }
    /// Generate an html report from this [`AnalysisReport`].
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - underlying templates could not be compiled.
    ///   - the base file name could not be determined.
    ///   - the conversion
    pub fn generate_html(&self, path: &Path) -> OpmResult<()> {
        info!("Write html report to {}", path.display());
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
    pub node: String,
    /// node type
    pub node_type: String,
    /// properties of the node
    pub props: Vec<HtmlProperty>,
    /// uuid of the node (needed for constructing filenames)
    pub uuid: String,
}
