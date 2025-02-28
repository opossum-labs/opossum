use crate::{
    analyzers::AnalyzerType,
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
    optic_node::OpticNode,
    SceneryResources,
};
use log::warn;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct OpmDocument {
    #[serde(rename = "opm file version")]
    opm_file_version: String,
    #[serde(default)]
    scenery: NodeGroup,
    #[serde(default, rename = "global")]
    global_conf: Arc<Mutex<SceneryResources>>,
    #[serde(default)]
    analyzers: Vec<AnalyzerType>,
}
impl Default for OpmDocument {
    fn default() -> Self {
        Self {
            opm_file_version: env!("OPM_FILE_VERSION").to_string(),
            scenery: NodeGroup::default(),
            global_conf: Arc::new(Mutex::new(SceneryResources::default())),
            analyzers: vec![],
        }
    }
}
impl OpmDocument {
    /// Creates a new [`OpmDocument`].
    #[must_use]
    pub fn new(scenery: NodeGroup) -> Self {
        Self {
            scenery,
            ..Default::default()
        }
    }
    /// Create a new [`OpmDocument`] from an `.opm` file at the given path.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the given path is not found or readable.
    ///   - the parsing / deserialization of the file failed.
    pub fn from_file(path: &Path) -> OpmResult<Self> {
        let contents = fs::read_to_string(path).map_err(|e| {
            OpossumError::OpmDocument(format!("cannot read file {} : {}", path.display(), e))
        })?;
        let mut document: Self = serde_yaml::from_str(&contents)
            .map_err(|e| OpossumError::OpmDocument(format!("parsing of model failed: {e}")))?;
        if document.opm_file_version != env!("OPM_FILE_VERSION") {
            warn!("OPM file version does not match the used OPOSSUM version.");
            warn!(
                "read version '{}' <-> program file version '{}'",
                document.opm_file_version,
                env!("OPM_FILE_VERSION")
            );
            warn!("This file might haven been written by an older or newer version of OPOSSUM. The model import might not be correct.");
        }
        document.scenery.after_deserialization_hook()?;
        document
            .scenery
            .graph_mut()
            .update_global_config(&Some(document.global_conf.clone()));
        Ok(document)
    }
    /// Create a new [`OpmDocument`] from the given `.opm` file string.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parsing of the `.opm` file failed.
    pub fn from_string(file_string: &str) -> OpmResult<Self> {
        let mut document: Self = serde_yaml::from_str(file_string)
            .map_err(|e| OpossumError::OpmDocument(format!("parsing of model failed: {e}")))?;
        if document.opm_file_version != env!("OPM_FILE_VERSION") {
            warn!("OPM file version does not match the used OPOSSUM version.");
            warn!(
                "read version '{}' <-> program file version '{}'",
                document.opm_file_version,
                env!("OPM_FILE_VERSION")
            );
            warn!("This file might haven been written by an older or newer version of OPOSSUM. The model import might not be correct.");
        }
        document.scenery.after_deserialization_hook()?;
        document
            .scenery
            .graph_mut()
            .update_global_config(&Some(document.global_conf.clone()));
        Ok(document)
    }
    /// Save this [`OpmDocument`] to an `.opm` file with the given path
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the serialization of the document failed.
    ///   - the file path cannot be created.
    ///   - it cannot write into the file (e.g. no space).
    pub fn save_to_file(&self, path: &Path) -> OpmResult<()> {
        let serialized = serde_yaml::to_string(&self).map_err(|e| {
            OpossumError::OpticScenery(format!("serialization of OpmDocument failed: {e}"))
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
    /// Return the content of the `.opm` file from this [`OpmDocument`]
    ///
    /// # Errors
    ///
    /// This function will return an error if the serialization of the internal structures fail.
    pub fn to_opm_file_string(&self) -> OpmResult<String> {
        serde_yaml::to_string(&self).map_err(|e| {
            OpossumError::OpticScenery(format!("serialization of OpmDocument failed: {e}"))
        })
    }
    pub fn add_analyzer(&mut self, analyzer: AnalyzerType) {
        self.analyzers.push(analyzer);
    }
    pub fn scenery_mut(&mut self) -> &mut NodeGroup {
        &mut self.scenery
    }
    #[must_use]
    pub fn analyzers(&self) -> Vec<AnalyzerType> {
        self.analyzers.clone()
    }
    /// Returns a reference to the global config of this [`OpmDocument`].
    #[must_use]
    pub fn global_conf(&self) -> &Mutex<SceneryResources> {
        &self.global_conf
    }
    /// Sets the global config of this [`OpmDocument`].
    pub fn set_global_conf(&mut self, rsrc: SceneryResources) {
        self.global_conf = Arc::new(Mutex::new(rsrc));
        self.scenery
            .graph_mut()
            .update_global_config(&Some(self.global_conf.clone()));
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            ghostfocus::GhostFocusAnalyzer, raytrace::RayTracingAnalyzer, Analyzer,
            GhostFocusConfig, RayTraceConfig,
        },
        degree, joule, millimeter, nanometer,
        nodes::{
            round_collimated_ray_source, BeamSplitter, CylindricLens, Dummy, EnergyMeter,
            FluenceDetector, IdealFilter, Lens, ParabolicMirror, ParaxialSurface,
            RayPropagationVisualizer, ReflectiveGrating, Spectrometer, SpotDiagram, ThinMirror,
            WaveFront, Wedge,
        },
        optic_node::OpticNode,
        utils::test_helper::test_helper::check_logs,
    };
    use petgraph::adj::NodeIndex;
    use std::{
        path::PathBuf,
        sync::{Arc, Mutex},
    };
    use tempfile::NamedTempFile;

    #[test]
    fn new() {
        let mut scenery = NodeGroup::default();
        scenery.node_attr_mut().set_name("MyTest");
        let document = OpmDocument::new(scenery);
        assert_eq!(document.scenery.node_attr().name(), "MyTest");
        assert!(document.analyzers.is_empty());
    }
    #[test]
    fn default() {
        let document = OpmDocument::default();
        assert_eq!(document.opm_file_version, env!("OPM_FILE_VERSION"));
        assert!(document.analyzers.is_empty());
    }

    #[test]
    fn from_file() {
        let result =
            OpmDocument::from_file(&Path::new("./invalid_file_path/invalid_file.invalid_ext"));
        assert!(result.unwrap_err().to_string().starts_with(
            "OpmDocument:cannot read file ./invalid_file_path/invalid_file.invalid_ext"
        ));
        let result =
            OpmDocument::from_file(&Path::new("./files_for_testing/opm/incorrect_opm.opm"));
        assert_eq!(
            result.unwrap_err().to_string(),
            "OpmDocument:parsing of model failed: missing field `opm file version`"
        );

        let document =
            OpmDocument::from_file(&PathBuf::from("./files_for_testing/opm/opticscenery.opm"))
                .unwrap();
        let node1 = document.scenery.node(NodeIndex::from(0)).unwrap();
        let node2 = document.scenery.node(NodeIndex::from(1)).unwrap();
        assert_eq!(
            "587fa699-5e98-4d08-b5a5-f9885151f3d1",
            node1.uuid().to_string()
        );
        assert_eq!(
            "a81f485c-26f7-4b3c-a6ac-4a62746f6cad",
            node2.uuid().to_string()
        );
    }
    #[test]
    fn save_to_file() {
        let file = NamedTempFile::new().unwrap();
        let path = file.into_temp_path();
        let document = OpmDocument::default();
        assert!(document.save_to_file(&path).is_ok());
        path.close().unwrap()
    }
    #[test]
    fn add_analyzer() {
        let mut document = OpmDocument::default();
        assert!(document.analyzers.is_empty());
        document.add_analyzer(AnalyzerType::Energy);
        assert_eq!(document.analyzers.len(), 1);
    }
    #[test]
    fn analyzers() {
        let mut document = OpmDocument::default();
        document.add_analyzer(AnalyzerType::Energy);
        document.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
        assert_eq!(document.analyzers().len(), 2);
    }
    #[test]
    fn all_nodes_integration_test() {
        let mut scenery = NodeGroup::default();
        let src = round_collimated_ray_source(millimeter!(10.0), joule!(1.0), 1).unwrap();
        let i_0 = scenery.add_node(&src).unwrap();
        let i_1 = scenery.add_node(&BeamSplitter::default()).unwrap();
        let i_2 = scenery.add_node(&CylindricLens::default()).unwrap();
        let i_3 = scenery.add_node(&FluenceDetector::default()).unwrap();
        let i_4 = scenery.add_node(&Lens::default()).unwrap();
        let i_5 = scenery.add_node(&Wedge::default()).unwrap();
        let i_6 = scenery.add_node(&Dummy::default()).unwrap();
        let i_7 = scenery.add_node(&EnergyMeter::default()).unwrap();
        let i_8 = scenery.add_node(&IdealFilter::default()).unwrap();
        let i_9 = scenery
            .add_node(&ParaxialSurface::new("paraxial", millimeter!(1000.0)).unwrap())
            .unwrap();
        let i_10 = scenery
            .add_node(&RayPropagationVisualizer::default())
            .unwrap();
        let i_11 = scenery.add_node(&Spectrometer::default()).unwrap();
        let i_12 = scenery.add_node(&SpotDiagram::default()).unwrap();
        let i_13 = scenery.add_node(&WaveFront::default()).unwrap();
        let i_14 = scenery.add_node(&ParabolicMirror::default()).unwrap();
        let i_15 = scenery
            .add_node(
                &ReflectiveGrating::default()
                    .with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))
                    .unwrap(),
            )
            .unwrap();
        let i_16 = scenery.add_node(&ThinMirror::default()).unwrap();

        scenery
            .connect_nodes(i_0, "output_1", i_1, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_1, "out1_trans1_refl2", i_2, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_2, "output_1", i_3, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_3, "output_1", i_4, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_4, "output_1", i_5, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_5, "output_1", i_6, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_6, "output_1", i_7, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_7, "output_1", i_8, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_8, "output_1", i_9, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_9, "output_1", i_10, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_10, "output_1", i_11, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_11, "output_1", i_12, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_12, "output_1", i_13, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_13, "output_1", i_14, "input_1", millimeter!(5.0))
            .unwrap();
        scenery
            .connect_nodes(i_14, "output_1", i_15, "input_1", millimeter!(50.0))
            .unwrap();
        scenery
            .connect_nodes(i_15, "output_1", i_16, "input_1", millimeter!(50.0))
            .unwrap();

        scenery.set_global_conf(Some(Arc::new(Mutex::new(SceneryResources::default()))));
        // Perform ray tracing analysis
        testing_logger::setup();
        let analyzer = RayTracingAnalyzer::new(RayTraceConfig::default());
        analyzer.analyze(&mut scenery).unwrap();
        check_logs(log::Level::Warn, vec![]);
        scenery.reset_data();
        // Perform ghost focus analysis
        let analyzer = GhostFocusAnalyzer::new(GhostFocusConfig::default());
        analyzer.analyze(&mut scenery).unwrap();
        check_logs(log::Level::Warn, vec![]);
        // let mut doc = OpmDocument::new(scenery);
        // // doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
        // doc.add_analyzer(AnalyzerType::GhostFocus(GhostFocusConfig::default()));
        // doc.save_to_file(Path::new(
        //     "../opossum/playground/all_nodes_integration_test.opm",
        // ))
        // .unwrap();
    }
}
