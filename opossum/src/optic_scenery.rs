//! The basic structure containing the entire optical model
use crate::{
    analyzers::AnalyzerType,
    error::{OpmResult, OpossumError},
    get_version,
    optic_graph::OpticGraph,
    optic_ref::OpticRef,
    optic_senery_rsc::SceneryResources,
    optical::{LightResult, Optical},
    reporter::AnalysisReport,
};
use chrono::Local;
use image::{DynamicImage, ImageReader};
use log::warn;

use petgraph::prelude::NodeIndex;
use serde::{
    de::{self, MapAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Serialize,
};
use std::{
    cell::RefCell,
    fs::File,
    io::{Cursor, Write},
    path::Path,
    rc::Rc,
};
use tempfile::NamedTempFile;
use uom::si::f64::Length;

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
/// use opossum::millimeter;
///
/// fn main() -> OpmResult<()> {
///   let mut scenery = OpticScenery::default();
///   scenery.set_description("OpticScenery demo");
///   let node1 = scenery.add_node(Dummy::new("dummy1"));
///   let node2 = scenery.add_node(Dummy::new("dummy2"));
///   scenery.connect_nodes(node1, "rear", node2, "front", millimeter!(100.0))?;
///   Ok(())
/// }
///
/// ```
#[derive(Debug, Clone)]
pub struct OpticScenery {
    g: OpticGraph,
    description: String,
    global_conf: Rc<RefCell<SceneryResources>>,
}
impl Default for OpticScenery {
    fn default() -> Self {
        Self {
            g: OpticGraph::default(),
            description: String::new(),
            global_conf: Rc::new(RefCell::new(SceneryResources::default())),
        }
    }
}
impl OpticScenery {
    /// Creates a new (empty) [`OpticScenery`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Add a given [`Optical`] (Source, Detector, Lens, etc.) to this [`OpticScenery`].
    ///
    /// This command just adds an [`Optical`] to the graph. It does not connect
    /// it to existing nodes in the graph. The given optical element is consumed (owned) by the [`OpticScenery`].
    /// This function returns a reference to the element in the scenery as [`NodeIndex`]. This reference must be used lateron
    /// for connecting nodes (see `connect_nodes` function).
    ///
    /// ## Panics
    ///
    /// This function might panic, if the underlying graph is set to `inverted`
    pub fn add_node<T: Optical + 'static>(&mut self, node: T) -> NodeIndex {
        self.g.add_node(node).unwrap()
    }
    /// Connect (already existing) optical nodes within this [`OpticScenery`].
    ///
    /// This function connects two optical nodes (referenced by their [`NodeIndex`]) with their respective port names and their geometrical distance
    /// (= propagation length) to each other thus extending the network.
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the [`NodeIndex`] of source or target node does not exist in the `OpticGraph`
    ///   - a port name of the source or target node does not exist
    ///   - if a node/port combination was already connected earlier
    ///   - the connection of the nodes would form a loop in the network.
    ///   - the given geometric distance between the nodes is not finite.
    pub fn connect_nodes(
        &mut self,
        src_node: NodeIndex,
        src_port: &str,
        target_node: NodeIndex,
        target_port: &str,
        distance: Length,
    ) -> OpmResult<()> {
        self.g
            .connect_nodes(src_node, src_port, target_node, target_port, distance)
    }
    /// Return a reference to the optical node specified by its [`NodeIndex`].
    ///
    /// This function is mainly useful for setting up a [reference node](crate::nodes::NodeReference).
    ///
    /// # Errors
    ///
    /// This function will return [`OpossumError::OpticScenery`] if the node does not exist.
    pub fn node(&self, node_idx: NodeIndex) -> OpmResult<OpticRef> {
        self.g.node_by_idx(node_idx)
    }
    /// Returns a vector of node references of this [`OpticScenery`].
    #[must_use]
    pub fn nodes(&self) -> Vec<&OpticRef> {
        self.g.nodes()
    }
    /// Export the optic graph, including ports, into the `dot` format to be used in combination with
    /// the [`graphviz`](https://graphviz.org/) software.
    ///
    /// # Errors
    /// This function returns an error if nodes do not return a proper value for their `name` property.
    pub fn to_dot(&self, rankdir: &str) -> OpmResult<String> {
        let mut dot_string = self.add_dot_header(rankdir);
        dot_string += &self.g.create_dot_string(rankdir)?;
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
        let img = ImageReader::new(Cursor::new(r.stdout))
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
    /// This function returns a string of a SVG image (scalable vector graphics). This string can be directly written to a
    /// `*.svg` file.
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
            .arg("-Tsvg:cairo")
            .arg("-Kdot")
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
        dot_string.push_str("\tfontname=\"Courier-monospace\"\n");
        dot_string.push_str("\tnode [fontname=\"Courier-monospace\" fontsize = 8]\n");
        dot_string.push_str("\tedge [fontname=\"Courier-monospace\"]\n\n");
        dot_string
    }
    /// Analyze this [`OpticScenery`] based on a given [`AnalyzerType`].
    ///
    /// # Attributes
    /// * `analyzer_type`: specified analyzer for this optical setup
    ///
    /// # Errors
    /// This function returns an error if an underlying node-specific analysis function returns an error.
    pub fn analyze(&mut self, analyzer_type: &AnalyzerType) -> OpmResult<()> {
        let description = self.description().to_string();
        if let AnalyzerType::RayTrace(_) = analyzer_type {
            let name = format!("scenery '{description}'");
            self.g.calc_node_positions(&name, &LightResult::default())?;
        }
        let name = format!("Scenery '{description}'");
        self.g
            .analyze(&name, &LightResult::default(), analyzer_type)?;
        Ok(())
    }
    /// Sets the description of this [`OpticScenery`].
    pub fn set_description(&mut self, description: &str) {
        description.clone_into(&mut self.description);
    }
    /// Returns a reference to the description of this [`OpticScenery`].
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
    /// Generate an [`AnalysisReport`] containing the result of an analysis.
    ///
    /// This [`AnalysisReport`] can then be used to either save it to disk or produce an HTML document from. In addition,
    /// the given report folder is used for the individual nodes to export specific result files.
    ///
    /// # Errors
    ///
    /// This function will return an error if the individual export function of a node fails.
    pub fn report(&self) -> OpmResult<AnalysisReport> {
        let mut analysis_report = AnalysisReport::new(get_version(), Local::now());
        analysis_report.add_scenery(self);
        let detector_nodes = self
            .g
            .nodes()
            .into_iter()
            .filter(|node| node.optical_ref.borrow().is_detector());
        for node in detector_nodes {
            let uuid = node.uuid().as_simple().to_string();
            if let Some(node_report) = node.optical_ref.borrow().report(&uuid) {
                analysis_report.add_detector(node_report);
            }
        }
        Ok(analysis_report)
    }
    /// Write node specific data files to the given `data_dir`.
    ///
    /// # Errors
    ///
    /// This function will return an error if the underlying `export_data` function of the corresponding
    /// node returns an error.
    pub fn export_node_data(&self, data_dir: &Path) -> OpmResult<()> {
        for node in self.g.nodes() {
            let uuid = node.uuid().as_simple().to_string();
            node.optical_ref.borrow().export_data(data_dir, &uuid)?;
        }
        Ok(())
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
    #[must_use]
    pub fn global_conf(&self) -> &RefCell<SceneryResources> {
        &self.global_conf
    }
    pub fn set_global_conf(&mut self, rsrc: SceneryResources) {
        self.global_conf = Rc::new(RefCell::new(rsrc));
        self.g.update_global_config(&Some(self.global_conf.clone()));
    }
}

impl Serialize for OpticScenery {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut scene = serializer.serialize_struct("scenery", 4)?;
        scene.serialize_field("opm version", &env!("OPM_FILE_VERSION"))?;
        scene.serialize_field("graph", &self.g)?;
        scene.serialize_field("description", &self.description)?;
        let global_conf = self.global_conf.borrow().to_owned();
        scene.serialize_field("global", &global_conf)?;
        scene.end()
    }
}
impl<'de> Deserialize<'de> for OpticScenery {
    #[allow(clippy::too_many_lines)]
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            OpmVersion,
            Graph,
            Description,
            Global,
        }
        const FIELDS: &[&str] = &["opm version", "graph", "properties", "global"];

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
                        formatter.write_str("`opm version`, `graph`, `description`, or `global`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "opm version" => Ok(Field::OpmVersion),
                            "graph" => Ok(Field::Graph),
                            "description" => Ok(Field::Description),
                            "global" => Ok(Field::Global),
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
                let mut description: Option<String> = None;
                let mut global_conf: Option<SceneryResources> = None;
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
                        Field::Description => {
                            if description.is_some() {
                                return Err(de::Error::duplicate_field("description"));
                            }
                            description = Some(map.next_value()?);
                        }
                        Field::Global => {
                            if global_conf.is_some() {
                                return Err(de::Error::duplicate_field("global"));
                            }
                            global_conf = Some(map.next_value()?);
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
                let description = description.unwrap_or_default();
                let global_conf = global_conf.unwrap_or_else(|| {
                    warn!("no global resources found, using default");
                    SceneryResources::default()
                });

                let mut s = OpticScenery {
                    g: graph,
                    description,
                    global_conf: Rc::new(RefCell::new(SceneryResources::default())),
                };
                s.set_global_conf(global_conf);
                Ok(s)
            }
        }
        deserializer.deserialize_struct("OpticScenery", FIELDS, OpticSceneryVisitor)
    }
}
#[cfg(test)]
mod test {
    use crate::{
        analyzers::{AnalyzerType, RayTraceConfig},
        joule,
        lightdata::LightData,
        millimeter, nanometer,
        nodes::{Detector, Dummy, EnergyMeter, Source},
        optical::Optical,
        properties::Proptype,
        ray::Ray,
        rays::Rays,
        utils::geom_transformation::Isometry,
        OpticScenery,
    };
    use num::Zero;
    use std::path::{Path, PathBuf};
    use tempfile::NamedTempFile;
    use uom::si::f64::Length;

    #[test]
    fn default() {
        let scenery = OpticScenery::default();
        assert_eq!(scenery.description(), "");
        assert_eq!(scenery.g.edge_count(), 0);
        assert_eq!(scenery.g.node_count(), 0);
    }
    #[test]
    fn new() {
        let scenery = OpticScenery::new();
        assert_eq!(scenery.description(), "");
        assert_eq!(scenery.g.edge_count(), 0);
        assert_eq!(scenery.g.node_count(), 0);
    }
    #[test]
    fn set_description() {
        let mut scenery = OpticScenery::default();
        assert_eq!(scenery.description(), "");
        scenery.set_description("Test");
        assert_eq!(scenery.description(), "Test")
    }
    #[test]
    fn save_to_file() {
        let scenery = OpticScenery::default();
        assert!(scenery.save_to_file(Path::new("")).is_err());
        let path = NamedTempFile::new().unwrap();
        assert!(scenery.save_to_file(path.path()).is_ok());
    }
    #[test]
    fn save_to_file_invalid_path() {
        let scenery = OpticScenery::default();
        assert!(scenery
            .save_to_file(&PathBuf::from(
                "./invalid_file_path/invalid_file.invalid_ext"
            ))
            .is_err());
    }
    #[test]
    fn analyze_dummy() {
        let mut scenery = OpticScenery::default();
        let node1 = scenery.add_node(Dummy::default());
        let node2 = scenery.add_node(Dummy::default());
        scenery
            .connect_nodes(node1, "rear", node2, "front", Length::zero())
            .unwrap();
        scenery.analyze(&AnalyzerType::Energy).unwrap();
    }
    #[test]
    fn analyze_empty() {
        let mut scenery = OpticScenery::default();
        scenery.analyze(&AnalyzerType::Energy).unwrap();
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
        let mut scenery = OpticScenery::default();
        let i_s = scenery.add_node(Source::new("src", &LightData::Geometric(rays)));
        let mut em = EnergyMeter::default();
        em.set_isometry(Isometry::identity());
        let i_e = scenery.add_node(em);
        scenery
            .connect_nodes(i_s, "out1", i_e, "in1", Length::zero())
            .unwrap();
        let mut raytrace_config = RayTraceConfig::default();
        raytrace_config.set_min_energy_per_ray(joule!(0.5)).unwrap();
        scenery
            .analyze(&AnalyzerType::RayTrace(raytrace_config))
            .unwrap();
        let uuid = scenery.node(i_e).unwrap().uuid().as_simple().to_string();
        let report = scenery
            .node(i_e)
            .unwrap()
            .optical_ref
            .borrow()
            .report(&uuid)
            .unwrap();
        if let Proptype::Energy(e) = report.properties().get("Energy").unwrap() {
            assert_eq!(*e, joule!(1.0));
        } else {
            assert!(false)
        }
    }
    #[test]
    fn report() {
        let mut scenery = OpticScenery::default();
        scenery.add_node(Detector::default());
        let report = scenery.report().unwrap();
        assert!(serde_yaml::to_string(&report).is_ok());
        // How shall we further parse the output?
    }
    #[test]
    fn report_empty() {
        let mut scenery = OpticScenery::default();
        scenery.analyze(&AnalyzerType::Energy).unwrap();
        scenery.report().unwrap();
    }
}
