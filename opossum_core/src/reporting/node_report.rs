//! Module for storing node specific data to be integrated in an [`AnalysisReport`](crate::reporting::analysis_report::AnalysisReport).
use std::path::Path;

use crate::{
    error::OpmResult,
    properties::{Properties, Proptype},
    reporting::report_note::ReportNote,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
/// Structure for storing node specific data to be integrated in an [`AnalysisReport`](crate::reporting::analysis_report::AnalysisReport).
pub struct NodeReport {
    node_type: String,
    name: String,
    uuid: String,
    properties: Properties,
    show_item: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    notes: Vec<ReportNote>,
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
            notes: Vec::new(),
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
    #[allow(clippy::missing_const_for_fn)]
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
    /// Return wether an item should be shown in a report or hidden.
    ///
    /// This function is necessary in order to hide or unhide node reports when exporting to Html.
    /// The Html template uses a (Bootstrap 5) accordion display, where item can be shown or hidden.
    #[must_use]
    pub const fn show_item(&self) -> bool {
        self.show_item
    }
    /// Sets wether a [`NodeReport`] should be displayed or hidden by default (see above).
    pub const fn set_show_item(&mut self, show_item: bool) {
        self.show_item = show_item;
    }
    /// Add a note to this [`NodeReport`].
    pub fn add_note(&mut self, note: ReportNote) {
        self.notes.push(note);
    }
    /// Returns a reference to the notes of this [`NodeReport`].
    #[must_use]
    pub fn notes(&self) -> &[ReportNote] {
        &self.notes
    }
    pub fn export(&self, report_path: &Path) -> OpmResult<()> {
        for (prop_id, prop) in self.properties().props_with_report_id_iter(&self.uuid) {
            prop.export_data(report_path, &prop_id)?;
        }
        Ok(())
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
        assert!(report.notes().is_empty());
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
