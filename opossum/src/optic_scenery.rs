//! The basic structure containing the entire optical model
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Cursor, Write};
use std::path::Path;

use crate::analyzer::AnalyzerType;
use crate::error::{OpmResult, OpossumError};
use crate::get_version;
use crate::light::Light;
use crate::lightdata::LightData;
use crate::nodes::NodeGroup;
use crate::optic_graph::OpticGraph;
use crate::optic_ref::OpticRef;
use crate::optical::{LightResult, Optical};
use crate::properties::{Properties, Proptype};
use crate::reporter::{AnalysisReport, PdfReportable};
use chrono::Local;
use genpdf::Alignment;
use image::io::Reader;
use image::DynamicImage;
use petgraph::algo::toposort;
use petgraph::prelude::NodeIndex;
use petgraph::visit::EdgeRef;
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

/// Overall optical model and additional metadata.
///
/// All optical elements ([`Optical`]s) have to be added to this structure in order
/// to be considered for an analysis.
///
/// # Example
///
/// ```rust
/// use opossum::OpticScenery;
/// use opossum::nodes::Dummy;
/// use opossum::error::OpmResult;
///
/// fn main() -> OpmResult<()> {
///   let mut scenery = OpticScenery::new();
///   scenery.set_description("OpticScenery demo");
///   let node1 = scenery.add_node(Dummy::new("dummy1"));
///   let node2 = scenery.add_node(Dummy::new("dummy2"));
///   scenery.connect_nodes(node1, "rear", node2, "front")
/// }
///
/// ```
#[derive(Debug, Clone)]
pub struct OpticScenery {
    g: OpticGraph,
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props
        .create("description", "title of the scenery", None, "".into())
        .unwrap();
    props
}

impl Default for OpticScenery {
    fn default() -> Self {
        Self {
            g: OpticGraph::default(),
            props: create_default_props(),
        }
    }
}
impl OpticScenery {
    /// Creates a new (empty) [`OpticScenery`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a given [`Optical`] (Source, Detector, Lens, etc.) to the graph of this [`OpticScenery`].
    ///
    /// This command just adds an [`Optical`] to the graph. It does not connect
    /// it to existing nodes in the graph. The given optical element is consumed (owned) by the [`OpticScenery`].
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> NodeIndex {
        self.g.add_node(node)
    }
    /// Connect (already existing) nodes denoted by the respective `NodeIndex`.
    ///
    /// # Errors
    /// Both node indices must exist. Otherwise an [`OpossumError::OpticScenery`] is returned. In addition, connections are
    /// rejected and an [`OpossumError::OpticScenery`] is returned, if the graph would form a cycle (loop in the graph).
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
    ) -> OpmResult<()> {
        self.g
            .connect_nodes(src_node, src_port, target_node, target_port)
    }
    /// Return a reference to the optical node specified by its node index.
    ///
    /// This function is mainly useful for setting up a reference node.
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node(&self, node: NodeIndex) -> OpmResult<OpticRef> {
        let node = self
            .g
            .0
            .node_weight(node)
            .ok_or_else(|| OpossumError::OpticScenery("node index does not exist".into()))?;
        Ok(node.clone())
    }
    /// Export the optic graph, including ports, into the `dot` format to be used in combination with the [`graphviz`](https://graphviz.org/) software.
    ///
    /// # Errors
    /// This function returns an error nodes do not return a proper value for their `name` property.
    pub fn to_dot(&self, rankdir: &str) -> OpmResult<String> {
        //check direction
        let rankdir = if rankdir == "LR" { "LR" } else { "TB" };

        let mut dot_string = self.add_dot_header(rankdir);

        for node_idx in self.g.0.node_indices() {
            let node = self
                .g
                .0
                .node_weight(node_idx)
                .ok_or_else(|| OpossumError::Other("could not get node_weigth".into()))?;
            let node_name = node.optical_ref.borrow().properties().name()?.to_owned();
            let inverted = node.optical_ref.borrow().properties().inverted()?;
            let ports = node.optical_ref.borrow().ports();
            dot_string += &node.optical_ref.borrow().to_dot(
                &format!("{}", node_idx.index()),
                &node_name,
                inverted,
                &ports,
                String::new(),
                rankdir,
            )?;
        }
        for edge in self.g.0.edge_indices() {
            let light: &Light = self
                .g
                .0
                .edge_weight(edge)
                .ok_or_else(|| OpossumError::Other("could not get node_weigth".into()))?;
            let end_nodes = self
                .g
                .0
                .edge_endpoints(edge)
                .ok_or_else(|| OpossumError::Other("could not get edge_endpoints".into()))?;

            let src_edge_str =
                self.create_node_edge_str(end_nodes.0, light.src_port(), String::new())?;
            let target_edge_str =
                self.create_node_edge_str(end_nodes.1, light.target_port(), String::new())?;

            dot_string.push_str(&format!("  {src_edge_str} -> {target_edge_str} \n"));
        }
        dot_string.push_str("}\n");
        Ok(dot_string)
    }
    /// Generate a [`DynamicImage`] of the [`OpticScenery`] diagram.
    ///
    /// # Errors
    ///
    /// This function will return an error if the image generation failes (e.g. no memory left etc.).
    pub fn to_dot_img(&self) -> OpmResult<DynamicImage> {
        let dot_string = self.to_dot("")?;
        let mut f = NamedTempFile::new()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        f.write_all(dot_string.as_bytes())
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        let r = std::process::Command::new("dot")
            .arg(f.path())
            .arg("-Tpng")
            .output()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        let img = Reader::new(Cursor::new(r.stdout))
            .with_guessed_format()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?
            .decode()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?
            .into_rgb8();
        let img = DynamicImage::ImageRgb8(img);
        Ok(img)
    }
    /// Returns the dot-file header of this [`OpticScenery`] graph.
    fn add_dot_header(&self, rankdir: &str) -> String {
        let mut dot_string = String::from("digraph {\n\tfontsize = 8\n");
        dot_string.push_str("\tsize = 5.0;\n");
        dot_string.push_str("\tdpi = 400.0;\n");
        dot_string.push_str("\tcompound = true;\n");
        dot_string.push_str(&format!("\trankdir = \"{rankdir}\";\n"));
        dot_string.push_str(&format!("\tlabel=\"{}\"\n", self.description()));
        dot_string.push_str("\tfontname=\"Helvetica\"\n");
        dot_string.push_str("\tnode [fontname=\"Helvetica\" fontsize = 10]\n");
        dot_string.push_str("\tedge [fontname=\"Helvetica\"]\n\n");
        dot_string
    }
    fn create_node_edge_str(
        &self,
        end_node: NodeIndex,
        light_port: &str,
        mut parent_identifier: String,
    ) -> OpmResult<String> {
        let node = self.g.0.node_weight(end_node).unwrap().optical_ref.borrow();
        parent_identifier = if parent_identifier.is_empty() {
            format!("i{}", end_node.index())
        } else {
            format!("{}_i{}", &parent_identifier, end_node.index())
        };

        if node.properties().node_type()? == "group" {
            let group_node: &NodeGroup = node.as_group()?;
            Ok(group_node.get_mapped_port_str(light_port, &parent_identifier)?)
        } else {
            Ok(format!("i{}:{}", end_node.index(), light_port))
        }
    }

    /// Analyze this [`OpticScenery`] based on a given [`AnalyzerType`].
    ///
    /// # Attributes
    /// * `analyzer_type`: specified analyzer for this optical setup
    ///
    /// # Errors
    /// This function returns an error if an underlying node-specific analysis function returns an error.
    pub fn analyze(&mut self, analyzer_type: &AnalyzerType) -> OpmResult<()> {
        print!("\nAnalyzing...");
        let _ = io::stdout().flush();
        let sorted = toposort(&self.g.0, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))?;
        for idx in sorted {
            let node = self
                .g
                .0
                .node_weight(idx)
                .ok_or_else(|| OpossumError::Analysis("getting node_weight failed".into()))?;
            let mut incoming_edges: HashMap<String, Option<LightData>> = self.incoming_edges(idx);
            // paranoia: check if all incoming ports are really input ports of the node to be analyzed
            let input_ports = node.optical_ref.borrow().ports().input_names();
            if !incoming_edges.iter().all(|e| input_ports.contains(e.0)) {
                return Err(OpossumError::Analysis("input light data contains port which is not an input port of the node. Data will be discarded.".into()));
            }
            incoming_edges = apodize_incoming_light(incoming_edges, node)?;
            //
            let node_name = node.optical_ref.borrow().properties().name()?.to_owned();
            let node_type = node
                .optical_ref
                .borrow()
                .properties()
                .node_type()?
                .to_owned();
            let mut outgoing_edges = node
                .optical_ref
                .borrow_mut()
                .analyze(incoming_edges, analyzer_type)
                .map_err(|e| format!("analysis of node {node_name} <{node_type}> failed: {e}"))?;
            outgoing_edges = apodize_outgoing_light(outgoing_edges, node)?;
            for outgoing_edge in outgoing_edges {
                self.set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);
            }
        }

        println!("Success\n");
        Ok(())
    }
    /// Sets the description of this [`OpticScenery`].
    ///
    /// # Attributes
    /// `description`: Description of the [`OpticScenery`]
    ///
    /// # Errors
    /// This function will return an [`OpossumError`] if the property "description" can not be set via the method [`set()`](./properties/struct.Properties.html#method.set).
    pub fn set_description(&mut self, description: &str) -> OpmResult<()> {
        self.props.set("description", description.into())
    }
    /// Returns a reference to the description of this [`OpticScenery`].
    #[must_use]
    pub fn description(&self) -> &str {
        if let Ok(Proptype::String(dsc)) = self.props.get("description") {
            dsc
        } else {
            ""
        }
    }
    fn incoming_edges(&self, idx: NodeIndex) -> LightResult {
        let edges = self.g.0.edges_directed(idx, petgraph::Direction::Incoming);
        edges
            .into_iter()
            .map(|e| {
                (
                    e.weight().target_port().to_owned(),
                    e.weight().data().cloned(),
                )
            })
            .collect::<HashMap<String, Option<LightData>>>()
    }
    fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: &str, data: Option<LightData>) {
        let edges = self.g.0.edges_directed(idx, petgraph::Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_idx = edge_ref.id();
            let light = self.g.0.edge_weight_mut(edge_idx);
            if let Some(light) = light {
                light.set_data(data);
            }
        } // else outgoing edge not connected
    }
    /// Generate an [`AnalysisReport`] containing the result of an analysis.
    ///
    /// This [`AnalysisReport`] can then be used to either save it to disk or produce a PDF document from. In addition,
    /// the given report folder is used for the individual nodes to export specific result files.
    ///
    /// # Errors
    ///
    /// This function will return an error if the individual export function of a node fails.
    pub fn report(&self, report_dir: &Path) -> OpmResult<AnalysisReport> {
        let mut analysis_report = AnalysisReport::new(get_version(), Local::now());
        analysis_report.add_scenery(self);
        let detector_nodes = self
            .g
            .0
            .node_weights()
            .filter(|node| node.optical_ref.borrow().is_detector());
        for node in detector_nodes {
            if let Some(node_report) = node.optical_ref.borrow().report() {
                analysis_report.add_detector(node_report);
            }
            node.optical_ref.borrow().export_data(report_dir)?;
        }
        Ok(analysis_report)
    }
    /// Save this [`OpticScenery`] to an .opm file with the given path
    ///
    /// # Errors
    ///
    /// This function will return an error if the file path cannot be created or it cannot write into the file (e.g. no space).
    pub fn save_to_file(&self, path: &Path) -> OpmResult<()> {
        let serialized = serde_json::to_string_pretty(&self).map_err(|e| {
            OpossumError::OpticScenery(format!("deserialization of OpticScenery failed: {e}"))
        })?;
        let mut output = File::create(path).map_err(|e| {
            OpossumError::OpticScenery(format!(
                "could not create file path: {}: {}",
                path.display(),
                e
            ))
        })?;
        write!(output, "{serialized}").map_err(|e| {
            OpossumError::OpticScenery(format!(
                "writing to file path {} failed: {}",
                path.display(),
                e
            ))
        })?;
        Ok(())
    }
}
fn apodize_incoming_light(
    incoming_edges: HashMap<String, Option<LightData>>,
    node: &OpticRef,
) -> OpmResult<HashMap<String, Option<LightData>>> {
    let mut apodized_edges: HashMap<String, Option<LightData>> = HashMap::new();
    let ports = node.optical_ref.borrow().ports();
    let input_ports = ports.inputs();
    for edge in incoming_edges {
        if let Some(LightData::Geometric(rays)) = edge.1 {
            if let Some(aperture) = input_ports.get(&edge.0) {
                let mut apodized_rays = rays.clone();
                apodized_rays.apodize(aperture);
                apodized_edges.insert(edge.0, Some(LightData::Geometric(apodized_rays)));
            } else {
                return Err(OpossumError::OpticScenery(format!(
                    "input port {} not found",
                    edge.0
                )));
            }
        } else {
            apodized_edges.insert(edge.0, edge.1);
        }
    }
    Ok(apodized_edges)
}
fn apodize_outgoing_light(
    outgoing_edges: HashMap<String, Option<LightData>>,
    node: &OpticRef,
) -> OpmResult<HashMap<String, Option<LightData>>> {
    let mut apodized_edges: HashMap<String, Option<LightData>> = HashMap::new();
    let ports = node.optical_ref.borrow().ports();
    let outgoing_ports = ports.outputs();
    for edge in outgoing_edges {
        if let Some(LightData::Geometric(rays)) = edge.1 {
            if let Some(aperture) = outgoing_ports.get(&edge.0) {
                let mut apodized_rays = rays.clone();
                apodized_rays.apodize(aperture);
                apodized_edges.insert(edge.0, Some(LightData::Geometric(apodized_rays)));
            } else {
                return Err(OpossumError::OpticScenery(format!(
                    "input port {} not found",
                    edge.0
                )));
            }
        } else {
            apodized_edges.insert(edge.0, edge.1);
        }
    }
    Ok(apodized_edges)
}
impl Serialize for OpticScenery {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut scene = serializer.serialize_struct("scenery", 3)?;
        scene.serialize_field("opm version", &env!("OPM_FILE_VERSION"))?;
        scene.serialize_field("graph", &self.g)?;
        scene.serialize_field("properties", &self.props)?;
        scene.end()
    }
}
impl<'de> Deserialize<'de> for OpticScenery {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            OpmVersion,
            Graph,
            Properties,
        }
        const FIELDS: &[&str] = &["opm version", "graph", "properties"];

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("`opm version`, `graph`, or `properties`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "opm version" => Ok(Field::OpmVersion),
                            "graph" => Ok(Field::Graph),
                            "properties" => Ok(Field::Properties),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct OpticSceneryVisitor;

        impl<'de> Visitor<'de> for OpticSceneryVisitor {
            type Value = OpticScenery;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an OpticScenery")
            }
            fn visit_map<A>(self, mut map: A) -> std::result::Result<OpticScenery, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut opm_version: Option<String> = None;
                let mut graph: Option<OpticGraph> = None;
                let mut properties: Option<Properties> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::OpmVersion => {
                            if opm_version.is_some() {
                                return Err(de::Error::duplicate_field("opm version"));
                            }
                            opm_version = Some(map.next_value()?);
                        }
                        Field::Graph => {
                            if graph.is_some() {
                                return Err(de::Error::duplicate_field("graph"));
                            }
                            graph = Some(map.next_value()?);
                        }
                        Field::Properties => {
                            if properties.is_some() {
                                return Err(de::Error::duplicate_field("properties"));
                            }
                            properties = Some(map.next_value()?);
                        }
                    }
                }
                if let Some(opm_version) = opm_version {
                    if opm_version != env!("OPM_FILE_VERSION") {
                        println!(
                            "\nWarning: version mismatch! File version {}, Appplication version {}",
                            opm_version,
                            env!("OPM_FILE_VERSION")
                        );
                    }
                }
                let graph = graph.ok_or_else(|| de::Error::missing_field("graph"))?;
                let properties =
                    properties.ok_or_else(|| de::Error::missing_field("properties"))?;

                Ok(OpticScenery {
                    g: graph,
                    props: properties,
                })
            }
        }
        deserializer.deserialize_struct("OpticScenery", FIELDS, OpticSceneryVisitor)
    }
}
impl PdfReportable for OpticScenery {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut l = genpdf::elements::LinearLayout::vertical();
        let diagram = self.to_dot_img()?;
        let img = genpdf::elements::Image::from_dynamic_image(diagram)
            .map_err(|e| format!("failed to add diagram to report: {e}"))?
            .with_alignment(Alignment::Center);
        l.push(img);
        Ok(l)
    }
}
#[cfg(test)]
mod test {
    use super::super::nodes::{BeamSplitter, Dummy, EnergyMeter, Source};
    use super::*;
    use crate::nodes::{Detector, Metertype};
    use std::{fs::File, io::Read};
    #[test]
    fn new() {
        let scenery = OpticScenery::new();
        assert_eq!(scenery.description(), "");
        assert_eq!(scenery.g.0.edge_count(), 0);
        assert_eq!(scenery.g.0.node_count(), 0);
    }
    #[test]
    fn to_dot_empty() {
        let path = "files_for_testing/dot/to_dot_empty_TB.dot";
        let file_content_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_tb);

        let path = "files_for_testing/dot/to_dot_empty_LR.dot";
        let file_content_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_lr);

        let mut scenery = OpticScenery::new();
        scenery.set_description("Test".into()).unwrap();

        let scenery_dot_str_tb = scenery.to_dot("TB").unwrap();
        let scenery_dot_str_lr = scenery.to_dot("LR").unwrap();

        assert_eq!(file_content_tb.clone(), scenery_dot_str_tb);
        assert_eq!(file_content_lr.clone(), scenery_dot_str_lr);
    }
    #[test]
    fn to_dot_with_node() {
        let path = "./files_for_testing/dot/to_dot_w_node_TB.dot";
        let file_content_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_tb);

        let path = "./files_for_testing/dot/to_dot_w_node_LR.dot";
        let file_content_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_lr);

        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into()).unwrap();
        scenery.add_node(Dummy::new("Test"));

        let scenery_dot_str_tb = scenery.to_dot("TB").unwrap();
        let scenery_dot_str_lr = scenery.to_dot("LR").unwrap();

        assert_eq!(file_content_tb.clone(), scenery_dot_str_tb);
        assert_eq!(file_content_lr.clone(), scenery_dot_str_lr);
    }
    #[test]
    fn to_dot_full() {
        let path = "files_for_testing/dot/to_dot_full_TB.dot";
        let file_content_tb = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_tb);

        let path = "files_for_testing/dot/to_dot_full_LR.dot";
        let file_content_lr = &mut "".to_owned();
        let _ = File::open(path).unwrap().read_to_string(file_content_lr);

        let mut scenery = OpticScenery::new();
        scenery.set_description("SceneryTest".into()).unwrap();
        let i_s = scenery.add_node(Source::new("Source", &LightData::Fourier));
        let mut bs = BeamSplitter::new("test", 0.6).unwrap();
        bs.set_property("name", "Beam splitter".into()).unwrap();
        let i_bs = scenery.add_node(bs);
        let i_d1 = scenery.add_node(EnergyMeter::new(
            "Energy meter 1",
            Metertype::IdealEnergyMeter,
        ));
        let i_d2 = scenery.add_node(EnergyMeter::new(
            "Energy meter 2",
            Metertype::IdealEnergyMeter,
        ));

        scenery.connect_nodes(i_s, "out1", i_bs, "input1").unwrap();
        scenery
            .connect_nodes(i_bs, "out1_trans1_refl2", i_d1, "in1")
            .unwrap();
        scenery
            .connect_nodes(i_bs, "out2_trans2_refl1", i_d2, "in1")
            .unwrap();

        let scenery_dot_str_tb = scenery.to_dot("TB").unwrap();
        let scenery_dot_str_lr = scenery.to_dot("LR").unwrap();

        assert_eq!(file_content_tb.clone(), scenery_dot_str_tb);
        assert_eq!(file_content_lr.clone(), scenery_dot_str_lr);
    }
    #[test]
    fn description() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("Test").unwrap();
        assert_eq!(scenery.description(), "Test")
    }
    #[test]
    fn report() {
        let mut scenery = OpticScenery::new();
        scenery.add_node(Detector::default());
        let report = scenery.report(Path::new(""));
        assert!(report.is_ok());
        let report = report.unwrap();
        assert!(serde_json::to_string(&report).is_ok());
        // How shall we further parse the output?
    }
    #[test]
    fn save_to_file() {
        let scenery = OpticScenery::new();
        assert!(scenery.save_to_file(Path::new("")).is_err());
        let path = NamedTempFile::new().unwrap();
        assert!(scenery.save_to_file(path.path()).is_ok());
    }
    #[test]
    fn analyze_empty() {
        let mut scenery = OpticScenery::default();
        assert!(scenery.analyze(&AnalyzerType::Energy).is_ok());
    }
    #[test]
    fn analyze_simple() {
        let mut scenery = OpticScenery::default();
        let n1 = scenery.add_node(Dummy::default());
        let n2 = scenery.add_node(Dummy::default());
        scenery.connect_nodes(n1, "rear", n2, "front").unwrap();
        assert!(scenery.analyze(&AnalyzerType::Energy).is_ok());
    }
}
