#![warn(missing_docs)]
//! Module handling analysis reports and converting them to HTML.

use crate::{
    error::{OpmResult, OpossumError}, nodes::{ray_propagation_visualizer::RayPositionHistories, NodeGroup}, optic_node::OpticNode, properties::{Properties, Proptype}
};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use super::html_report::{HtmlNodeReport, HtmlReport};

#[derive(Serialize, Debug, Clone)]
/// Structure for storing data being integrated in an analysis report.
pub struct AnalysisReport {
    opossum_version: String,
    analysis_timestamp: DateTime<Local>,
    scenery: Option<NodeGroup>,
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
    /// Add an [`NodeGroup`] to this [`AnalysisReport`].
    ///
    /// This function is called internally by the top level [`NodeGroup`] for adding itself to the report.
    pub fn add_scenery(&mut self, scenery: &NodeGroup) {
        self.scenery = Some(scenery.clone());
    }
    /// Add an (detector) [`NodeReport`] to this [`AnalysisReport`].
    ///
    /// After analysis of a [`NodeGroup`], each node can generate a [`NodeReport`] using the
    /// [`report`](crate::optic_node::OpticNode::report) trait function. While assembling a report this
    /// function adds the node data to it. This is mostly interesting for detector nodes which deliver
    /// their particular analysis result.
    pub fn add_detector(&mut self, report: NodeReport) {
        self.node_reports.push(report);
    }
    /// Returns the ray history for the first found [`RayPropagationVisualizer`](crate::nodes::RayPropagationVisualizer) in this [`AnalysisReport`].
    /// **Note**: This function is only a hack for displaying rays in the bevy engine.
    #[must_use]
    pub fn get_ray_hist(&self) -> Option<&RayPositionHistories> {
        for node in &self.node_reports {
            if let Some(ray_hist) = node.get_ray_history() {
                return Some(ray_hist);
            }
        }
        None
    }
    /// Generate an [`HtmlReport`] from this [`AnalysisReport`].
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn to_html_report(&self) -> OpmResult<HtmlReport> {
        let Some(scenery) = &self.scenery else {
            return Err(OpossumError::Other("no scenery found".into()));
        };
        let html_node_reports: Vec<HtmlNodeReport> = self
            .node_reports
            .iter()
            .map(NodeReport::to_html_node_report)
            .collect();
        Ok(HtmlReport::new(
            self.opossum_version.clone(),
            self.analysis_timestamp.format("%Y/%m/%d %H:%M").to_string(),
            scenery.node_attr().name(),
            html_node_reports,
        ))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
/// Structure for storing node specific data to be integrated in the [`AnalysisReport`].
pub struct NodeReport {
    node_type: String,
    name: String,
    uuid: String,
    properties: Properties,
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
    pub fn to_html_node_report(&self) -> HtmlNodeReport {
        HtmlNodeReport {
            node: self.name.clone(),
            node_type: self.node_type.clone(),
            props: self.properties.html_props(self.name(), &self.uuid),
            uuid: self.uuid.clone(),
        }
    }
    /// Returns the ray history of this [`NodeReport`] if it describe either a ray propagation
    /// visualizer node or a group containing such a node. Otherwise the return value is `None`.
    ///
    /// **Note**: This is a temporary function to be used in combination with the Bevy visualizer.
    #[must_use]
    pub fn get_ray_history(&self) -> Option<&RayPositionHistories> {
        if self.node_type == "group" {
            for prop in &self.properties {
                if let Proptype::NodeReport(node) = prop.1.prop() {
                    let data = node.get_ray_history();
                    if data.is_some() {
                        return data;
                    }
                }
            }
        } else if self.node_type == "ray propagation" {
            if let Ok(Proptype::RayPositionHistory(ray_hist)) =
                self.properties.get("Ray Propagation visualization plot")
            {
                return Some(ray_hist);
            }
        }
        None
    }
    /// Returns a reference to the uuid of this [`NodeReport`].
    #[must_use]
    pub fn uuid(&self) -> &str {
        &self.uuid
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
        report.add_scenery(&NodeGroup::default());
        assert!(report.scenery.is_some());
    }
    #[test]
    fn analysis_report_add_detector() {
        let mut report = AnalysisReport::new(String::from("test"), DateTime::default());
        report.add_detector(NodeReport::new(
            "test detector",
            "detector name",
            "123",
            Properties::default(),
        ));
        assert_eq!(report.node_reports.len(), 1);
    }
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
