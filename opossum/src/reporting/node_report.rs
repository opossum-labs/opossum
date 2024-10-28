use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    error::OpmResult,
    properties::{Properties, Proptype},
};

use super::html_report::HtmlNodeReport;

#[derive(Serialize, Deserialize, Clone, Debug)]
/// Structure for storing node specific data to be integrated in the [`AnalysisReport`].
pub struct NodeReport {
    node_type: String,
    name: String,
    uuid: String,
    properties: Properties,
    show_item: bool,
}
impl NodeReport {
    /// Creates a new [`NodeReport`].
    #[must_use]
    pub fn new(node_type: &str, name: &str, uuid: &str, properties: Properties) -> Self {
        Self {
            node_type: node_type.to_owned(),
            name: name.to_owned(),
            uuid: uuid.to_string(),
            properties,
            show_item: false,
        }
    }
    /// Returns a reference to the node type of this [`NodeReport`].
    #[must_use]
    pub fn node_type(&self) -> &str {
        self.node_type.as_ref()
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
    /// Return an [`HtmlNodeReport`] from this [`NodeReport`].
    #[must_use]
    pub fn to_html_node_report(&self, id: &str) -> HtmlNodeReport {
        HtmlNodeReport {
            node_name: self.name.clone(),
            node_type: self.node_type.clone(),
            props: self
                .properties
                .html_props(&format!("{id}_{}_{}", self.name, self.uuid)),
            uuid: self.uuid.clone(),
            show_item: self.show_item,
        }
    }
    /// Returns a reference to the uuid of this [`NodeReport`].
    #[must_use]
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
    /// .
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn export_data(&self, report_path: &Path, id: &str) -> OpmResult<()> {
        self.properties
            .export_data(report_path, &format!("{id}_{}_{}", &self.name, &self.uuid))
    }
    #[must_use]
    pub const fn show_item(&self) -> bool {
        self.show_item
    }
    pub fn set_show_item(&mut self, show_item: bool) {
        self.show_item = show_item;
    }
}

impl From<NodeReport> for Proptype {
    fn from(value: NodeReport) -> Self {
        Self::NodeReport(value)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn node_report_new() {
        let report = NodeReport::new(
            "test detector",
            "detector name",
            "123",
            Properties::default(),
        );
        assert_eq!(report.node_type, "test detector");
        assert_eq!(report.name, "detector name");
    }
}
