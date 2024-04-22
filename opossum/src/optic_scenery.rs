//! The basic structure containing the entire optical model
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;

use crate::utils::geom_transformation::Isometry;
use crate::{
    analyzer::AnalyzerType,
    error::{OpmResult, OpossumError},
    get_version,
    light::Light,
    lightdata::LightData,
    nodes::NodeGroup,
    optic_graph::OpticGraph,
    optic_ref::OpticRef,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    reporter::AnalysisReport,
};
use chrono::Local;
use image::io::Reader;
use image::DynamicImage;
use log::warn;
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
        self.g.connect_nodes(
            src_node,
            src_port,
            target_node,
            target_port,
            Isometry::identity(),
        )
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
            let node_name = node.optical_ref.borrow().name();
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
    /// Generate an SVG of the [`OpticScenery`] diagram.
    ///
    /// # Errors
    ///
    /// This function will return an error if the image generation failes (e.g. program not found, no memory left etc.).
    pub fn to_dot_svg(&self) -> OpmResult<String> {
        let dot_string = self.to_dot("")?;
        let mut f = NamedTempFile::new()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        f.write_all(dot_string.as_bytes())
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        let r = std::process::Command::new("dot")
            .arg(f.path())
            .arg("-Tsvg")
            .output()
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        let svg_string = String::from_utf8(r.stdout)
            .map_err(|e| OpossumError::Other(format!("conversion to image failed: {e}")))?;
        Ok(svg_string)
    }
    /// Returns the dot-file header of this [`OpticScenery`] graph.
    fn add_dot_header(&self, rankdir: &str) -> String {
        let mut dot_string = String::from("digraph {\n\tfontsize = 8;\n");
        dot_string.push_str("\tcompound = true;\n");
        dot_string.push_str(&format!("\trankdir = \"{rankdir}\";\n"));
        dot_string.push_str(&format!("\tlabel=\"{}\"\n", self.description()));
        dot_string.push_str("\tfontname=\"Courier\"\n");
        dot_string.push_str("\tnode [fontname=\"Courier\" fontsize = 8]\n");
        dot_string.push_str("\tedge [fontname=\"Courier\"]\n\n");
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

        if node.node_type() == "group" {
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
        let is_single_tree = self.g.is_single_tree();
        if !is_single_tree {
            warn!("Scenery contains unconnected sub-trees. Analysis might not be complete.");
        }
        let sorted = toposort(&self.g.0, None)
            .map_err(|_| OpossumError::Analysis("topological sort failed".into()))?;
        for idx in sorted {
            let node = self
                .g
                .0
                .node_weight(idx)
                .ok_or_else(|| OpossumError::Analysis("getting node_weight failed".into()))?;
            let node_name = node.optical_ref.borrow().name();
            let neighbors = self.g.0.neighbors_undirected(idx);
            if neighbors.count() == 0 {
                warn!("stale (completely unconnected) node {node_name} found. Skipping.");
            } else {
                let incoming_edges: LightResult = self.incoming_edges(idx);
                // paranoia: check if all incoming ports are really input ports of the node to be analyzed
                let input_ports = node.optical_ref.borrow().ports().input_names();
                if !incoming_edges.iter().all(|e| input_ports.contains(e.0)) {
                    warn!("input light data contains port which is not an input port of the node {node_name}. Data will be discarded.");
                }
                //
                let node_type = node.optical_ref.borrow().node_type();
                let outgoing_edges = node
                    .optical_ref
                    .borrow_mut()
                    .analyze(incoming_edges, analyzer_type)
                    .map_err(|e| {
                        OpossumError::Analysis(format!(
                            "analysis of node {node_name} <{node_type}> failed: {e}"
                        ))
                    })?;
                // Warn, if empty output LightResult but node has output ports defined.
                if outgoing_edges.is_empty()
                    && !node.optical_ref.borrow().ports().outputs().is_empty()
                    && is_single_tree
                {
                    warn!("analysis of node {node_name} <{node_type}> did not result in any output data. This might come from wrong / empty input data.");
                }
                for outgoing_edge in outgoing_edges {
                    self.set_outgoing_edge_data(idx, &outgoing_edge.0, outgoing_edge.1);
                }
            }
        }
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
            .filter(|e| e.weight().data().is_some())
            .map(|e| {
                (
                    e.weight().target_port().to_owned(),
                    e.weight().data().cloned().unwrap(),
                )
            })
            .collect::<LightResult>()
    }
    fn set_outgoing_edge_data(&mut self, idx: NodeIndex, port: &str, data: LightData) {
        let edges = self.g.0.edges_directed(idx, petgraph::Direction::Outgoing);
        let edge_ref = edges
            .into_iter()
            .filter(|idx| idx.weight().src_port() == port)
            .last();
        if let Some(edge_ref) = edge_ref {
            let edge_idx = edge_ref.id();
            let light = self.g.0.edge_weight_mut(edge_idx);
            if let Some(light) = light {
                light.set_data(Some(data));
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
        let serialized = serde_yaml::to_string(&self).map_err(|e| {
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
                        warn!(
                            "model file version mismatch! File version {}, Appplication version {}",
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
#[cfg(test)]
mod test {

    use crate::{
        analyzer::{AnalyzerType, RayTraceConfig},
        joule,
        lightdata::LightData,
        millimeter, nanometer,
        nodes::{
            BeamSplitter, Detector, Dummy, EnergyMeter, IdealFilter, NodeReference,
            ParaxialSurface, Propagation, RayPropagationVisualizer, Source, Spectrometer,
            SpotDiagram, WaveFront,
        },
        optical::Optical,
        properties::Proptype,
        ray::Ray,
        rays::Rays,
        OpticScenery,
    };
    use log::Level;
    use std::path::{Path, PathBuf};
    use tempfile::NamedTempFile;
    use testing_logger;

    #[test]
    fn new() {
        let scenery = OpticScenery::new();
        assert_eq!(scenery.description(), "");
        assert_eq!(scenery.g.0.edge_count(), 0);
        assert_eq!(scenery.g.0.node_count(), 0);
    }
    #[test]
    fn description() {
        let mut scenery = OpticScenery::new();
        assert_eq!(scenery.description(), "");
        scenery.set_description("Test".into()).unwrap();
        assert_eq!(scenery.description(), "Test")
    }
    #[test]
    fn report() {
        let mut scenery = OpticScenery::new();
        scenery.add_node(Detector::default());
        let report = scenery.report(Path::new(""));
        assert!(report.is_ok());
        let report = report.unwrap();
        assert!(serde_yaml::to_string(&report).is_ok());
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
    fn analyze_dummy_test() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("analyze_dummy_test").unwrap();
        let node1 = scenery.add_node(Dummy::new("dummy1"));
        let node2 = scenery.add_node(Dummy::new("dummy2"));
        scenery
            .connect_nodes(node1, "rear", node2, "front")
            .unwrap();
        scenery.analyze(&AnalyzerType::Energy).unwrap();
    }
    #[test]
    fn analyze_empty_test() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("analyze_empty_test").unwrap();
        scenery.analyze(&AnalyzerType::Energy).unwrap();
    }
    #[test]
    fn analyze_stale_node() {
        testing_logger::setup();
        let mut scenery = OpticScenery::new();
        scenery.add_node(Dummy::default());
        assert!(scenery.analyze(&AnalyzerType::Energy).is_ok());
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            assert_eq!(
                captured_logs[0].body,
                "stale (completely unconnected) node dummy found. Skipping."
            );
            assert_eq!(captured_logs[0].level, Level::Warn);
        });
    }
    #[test]
    fn analyze_unconnected_sub_trees() {
        testing_logger::setup();
        let mut scenery = OpticScenery::new();
        let n1 = scenery.add_node(Dummy::default());
        let n2 = scenery.add_node(Dummy::default());
        let n3 = scenery.add_node(Dummy::default());
        let n4 = scenery.add_node(Dummy::default());
        scenery.connect_nodes(n1, "rear", n2, "front").unwrap();
        scenery.connect_nodes(n3, "rear", n4, "front").unwrap();
        assert!(scenery.analyze(&AnalyzerType::Energy).is_ok());
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            assert_eq!(
                captured_logs[0].body,
                "Scenery contains unconnected sub-trees. Analysis might not be complete."
            );
            assert_eq!(captured_logs[0].level, Level::Warn);
        });
    }
    #[test]
    fn analyze_energy_threshold() {
        let mut rays = Rays::default();
        rays.add_ray(
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap(),
        );
        rays.add_ray(
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(0.1)).unwrap(),
        );
        let mut scenery = OpticScenery::new();
        let i_s = scenery.add_node(Source::new("src", &LightData::Geometric(rays)));
        let i_e = scenery.add_node(EnergyMeter::default());
        scenery.connect_nodes(i_s, "out1", i_e, "in1").unwrap();
        let mut raytrace_config = RayTraceConfig::default();
        raytrace_config.set_min_energy_per_ray(joule!(0.5)).unwrap();
        scenery
            .analyze(&AnalyzerType::RayTrace(raytrace_config))
            .unwrap();
        let report = scenery
            .node(i_e)
            .unwrap()
            .optical_ref
            .borrow()
            .report()
            .unwrap();
        if let Proptype::Energy(e) = report.properties().get("Energy").unwrap() {
            assert_eq!(*e, joule!(1.0));
        } else {
            assert!(false)
        }
    }
    #[test]
    fn report_empty_test() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("report_empty_test").unwrap();
        scenery.analyze(&AnalyzerType::Energy).unwrap();
        let _report = scenery.report(&PathBuf::from("./opossum/files_for_testing/"));
    }

    #[test]
    fn save_to_file_invalid_path_test() {
        let mut scenery = OpticScenery::new();
        scenery.set_description("analyze_empty_test").unwrap();
        assert!(scenery
            .save_to_file(&PathBuf::from(
                "./invalid_file_path/invalid_file.invalid_ext"
            ))
            .is_err());
    }
    #[test]
    fn is_source_test() {
        assert!(!BeamSplitter::default().is_source());
        assert!(!Detector::default().is_source());
        assert!(!Dummy::default().is_source());
        assert!(!EnergyMeter::default().is_source());
        assert!(!IdealFilter::default().is_source());
        assert!(!ParaxialSurface::default().is_source());
        assert!(!Propagation::default().is_source());
        assert!(!RayPropagationVisualizer::default().is_source());
        assert!(!Spectrometer::default().is_source());
        assert!(!SpotDiagram::default().is_source());
        assert!(!WaveFront::default().is_source());
        assert!(!NodeReference::default().is_source());
        assert!(Source::default().is_source());

        let mut scenery = OpticScenery::default();
        let idx = scenery.add_node(Source::default());
        let idx2 = scenery.add_node(Detector::default());
        let node_ref = scenery.node(idx).unwrap();
        let src_ref = NodeReference::from_node(&node_ref);
        assert!(src_ref.is_source());

        let node_ref = scenery.node(idx2).unwrap();
        let not_src_ref = NodeReference::from_node(&node_ref);
        assert!(!not_src_ref.is_source());
    }
}
