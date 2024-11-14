use super::html_report::HtmlNodeReport;
use crate::{
    error::OpmResult,
    properties::{Properties, Proptype},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
/// Structure for storing node specific data to be integrated in an [`AnalysisReport`](crate::reporting::analysis_report::AnalysisReport).
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
    /// Returns a reference to the [`Properties`] of this [`NodeReport`].
    #[must_use]
    pub const fn properties(&self) -> &Properties {
        &self.properties
    }
    /// Returns a reference to the uuid of this [`NodeReport`].
    #[must_use]
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
    /// Return wether an item should be shown in a report or hidden.
    ///
    /// This function is necessary in order to hide or unhide node reports when exporting to Html.
    /// The Html template uses a (Bottstrap 5) accordion display, where item can be shown or hidden.
    #[must_use]
    pub const fn show_item(&self) -> bool {
        self.show_item
    }
    /// Sets wether a [`NodeReport`] should be displayed or hidden by default (see above).
    pub fn set_show_item(&mut self, show_item: bool) {
        self.show_item = show_item;
    }
    /// Return an [`HtmlNodeReport`] from this [`NodeReport`].
    ///
    /// This function is necessary, since TinyTemplates cannot deal with [`Properties`] correctly. Maybe this can be changes later.
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
    /// Export data files for the properties of this [`NodeReport`].
    ///
    /// This function exports data (mostly as data files) for each property. This is necessary if a report is exported to HTML.
    /// In this case, the [`HtmlNodeReport`] often only conatins a link to the corresponding data file (i.e. image of a plot).
    ///
    /// **Todo**: This function should be rather moved to the [`HtmlNodeReport`] struct.
    ///
    /// # Errors
    ///
    /// This function will return an error if the underlying export function of a property returns an error.
    pub fn export_data(&self, report_path: &Path, id: &str) -> OpmResult<()> {
        self.properties
            .export_data(report_path, &format!("{id}_{}_{}", &self.name, &self.uuid))
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
    fn new() {
        let report = NodeReport::new(
            "test detector",
            "detector name",
            "123",
            Properties::default(),
        );
        assert_eq!(report.node_type, "test detector");
        assert_eq!(report.name, "detector name");
        assert_eq!(report.uuid, "123");
        assert_eq!(report.properties.nr_of_props(), 0);
        assert_eq!(report.show_item, false);

        assert_eq!(report.node_type(), "test detector");
        assert_eq!(report.name(), "detector name");
        assert_eq!(report.uuid(), "123");
        assert_eq!(report.properties().nr_of_props(), 0);
    }
    #[test]
    fn show_item() {
        let mut report = NodeReport::new(
            "test detector",
            "detector name",
            "123",
            Properties::default(),
        );
        assert_eq!(report.show_item(), false);
        report.set_show_item(true);
        assert_eq!(report.show_item, true);
        assert_eq!(report.show_item(), true);
    }
    #[test]
    fn to_html_node_report() {
        let mut properties = Properties::default();
        properties
            .create("test1", "desc1", None, 1.0.into())
            .unwrap();
        properties
            .create("test2", "desc2", None, "test".into())
            .unwrap();
        let report = NodeReport::new("test detector", "detector name", "123", properties);
        let html_report = report.to_html_node_report("345");
        assert_eq!(html_report.node_name, "detector name");
        assert_eq!(html_report.node_type, "test detector");
        assert_eq!(html_report.uuid, "123");
        assert_eq!(html_report.show_item, false);
        let html_props = html_report.props;

        assert_eq!(html_props[0].name, "test1");
        assert_eq!(html_props[0].description, "desc1");
        assert_eq!(html_props[0].prop_value, "1.000000");
    }
    #[test]
    fn export_data() {
        let report = NodeReport::new(
            "test detector",
            "detector name",
            "123",
            Properties::default(),
        );
        assert!(report.export_data(Path::new("test"), "456").is_ok());
        // What else should / can we check here???
    }
    #[test]
    fn to_proptype() {
        let report = NodeReport::new(
            "test detector",
            "detector name",
            "123",
            Properties::default(),
        );
        let prop_type: Proptype = report.into();
        assert!(matches!(prop_type, Proptype::NodeReport(_)));
    }
}
