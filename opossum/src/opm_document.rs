#![warn(missing_docs)]
//! The basic structure of an OPOSSUM model.
//!
//! It contains the [`OpmDocument`] structure, which holds a (toplevel) [`NodeGroup`] representing the actual optical model
//! as well as a list of analyzers with their particular configuration and a global scene configuration (e.g. ambient medium etc.).
//!
//! This module also handles reading and writing of `.opm` files.
use crate::{
    analyzers::{
        energy::EnergyAnalyzer, ghostfocus::GhostFocusAnalyzer, raytrace::RayTracingAnalyzer,
        Analyzer, AnalyzerType,
    },
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
    optic_node::OpticNode,
    reporting::analysis_report::AnalysisReport,
    SceneryResources,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
/// The main structure of an OPOSSUM model.
/// It contains the [`NodeGroup`] representing the optical model, a list of analyzers and a global configuration.
pub struct OpmDocument {
    opm_file_version: String,
    #[serde(default)]
    scenery: NodeGroup,
    #[serde(default, rename = "global")]
    global_conf: Arc<Mutex<SceneryResources>>,
    #[serde(default)]
    analyzers: HashMap<Uuid, AnalyzerType>,
}
impl Default for OpmDocument {
    fn default() -> Self {
        Self {
            opm_file_version: env!("OPM_FILE_VERSION").to_string(),
            scenery: NodeGroup::default(),
            global_conf: Arc::new(Mutex::new(SceneryResources::default())),
            analyzers: HashMap::default(),
        }
    }
}
impl OpmDocument {
    /// Creates a new [`OpmDocument`].
    #[must_use]
    pub fn new(mut scenery: NodeGroup) -> Self {
        scenery.set_global_conf(Some(Arc::new(Mutex::new(SceneryResources::default()))));
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
        Self::from_string(&contents)
    }
    /// Create a new [`OpmDocument`] from the given `.opm` file string.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parsing of the `.opm` file failed.
    pub fn from_string(file_string: &str) -> OpmResult<Self> {
        let mut document: Self = ron::from_str(file_string)
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
        let serialized = self.to_opm_file_string()?;
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
        ron::ser::to_string_pretty(&self, ron::ser::PrettyConfig::new().new_line("\n")).map_err(
            |e| OpossumError::OpticScenery(format!("serialization of OpmDocument failed: {e}")),
        )
    }
    /// Returns the list of analyzers of this [`OpmDocument`].
    #[must_use]
    pub fn analyzers(&self) -> HashMap<Uuid, AnalyzerType> {
        self.analyzers.clone()
    }
    /// Return an [`AnalyzerType`] with the given [`Uuid`] from this [`OpmDocument`].
    ///
    /// # Errors
    ///
    /// This functions returns an error if the [`AnalyzerType`] with the given [`Uuid`] was not found.
    pub fn analyzer(&self, id: Uuid) -> OpmResult<AnalyzerType> {
        self.analyzers.get(&id).map_or_else(
            || {
                Err(OpossumError::OpmDocument(
                    "Analyzer with given Uuid not found.".into(),
                ))
            },
            |analyzer_type| Ok(analyzer_type.clone()),
        )
    }
    /// Add an analyzer to this [`OpmDocument`].
    pub fn add_analyzer(&mut self, analyzer: AnalyzerType) -> Uuid {
        let uuid = Uuid::new_v4();
        self.analyzers.insert(uuid, analyzer);
        uuid
    }
    /// Remove an analyzer from this [`OpmDocument`].
    ///
    /// This function removes an [`AnalyzerType`] with the given [`Uuid`] from this [`OpmDocument`].
    /// # Errors
    ///
    /// This function will return an error if an [`AnalyzerType`] with the given [`Uuid`] was not found.
    pub fn remove_analyzer(&mut self, id: Uuid) -> OpmResult<()> {
        if self.analyzers.remove(&id).is_some() {
            Ok(())
        } else {
            Err(OpossumError::OpmDocument(
                "Analyzer with given Uuid not found".into(),
            ))
        }
    }
    /// Returns a reference to the scenery of this [`OpmDocument`].
    #[must_use]
    pub const fn scenery(&self) -> &NodeGroup {
        &self.scenery
    }
    /// Returns a mutable reference to the scenery of this [`OpmDocument`].
    pub fn scenery_mut(&mut self) -> &mut NodeGroup {
        &mut self.scenery
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
    /// Perform an analysis run of this [`OpmDocument`].
    ///
    /// This function will perform the analysis of the defined analyzers in the order they were added.
    /// The results of the analysis will be returned as a vector of [`AnalysisReport`]s.
    ///
    /// # Errors
    ///
    /// This function will return an error if the individual analyzers fail to perform the analysis.
    pub fn analyze(&mut self) -> OpmResult<Vec<AnalysisReport>> {
        if self.analyzers.is_empty() {
            info!("No analyzer defined in document. Stopping here.");
            return Ok(vec![]);
        }
        let mut reports = vec![];
        for ana in self.analyzers.iter().enumerate() {
            let analyzer: &dyn Analyzer = match ana.1 .1 {
                AnalyzerType::Energy => &EnergyAnalyzer::default(),
                AnalyzerType::RayTrace(config) => &RayTracingAnalyzer::new(config.clone()),
                AnalyzerType::GhostFocus(config) => &GhostFocusAnalyzer::new(config.clone()),
            };
            info!("Analysis #{}", ana.0);
            analyzer.analyze(&mut self.scenery)?;
            reports.push(analyzer.report(&self.scenery)?);
            self.scenery.clear_edges();
            self.scenery.reset_data();
        }
        Ok(reports)
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
            collimated_line_ray_source, round_collimated_ray_source, BeamSplitter, CylindricLens,
            Dummy, EnergyMeter, FluenceDetector, IdealFilter, Lens, ParabolicMirror,
            ParaxialSurface, RayPropagationVisualizer, ReflectiveGrating, Spectrometer,
            SpotDiagram, ThinMirror, WaveFront, Wedge,
        },
        optic_node::{Alignable, OpticNode},
        refractive_index::RefrIndexConst,
        utils::test_helper::test_helper::check_logs,
    };
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
            "OpmDocument:parsing of model failed: 1:2: Unexpected missing field named `opm_file_version` in `OpmDocument`"
        );
        assert!(
            OpmDocument::from_file(&PathBuf::from("./files_for_testing/opm/opticscenery.opm"))
                .is_ok()
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
    fn analyzer() {
        let mut document = OpmDocument::default();
        let uuid1 = document.add_analyzer(AnalyzerType::Energy);
        let uuid2 = document.add_analyzer(AnalyzerType::Energy);

        assert!(document.analyzer(uuid1).is_ok());
        assert!(document.analyzer(uuid2).is_ok());
        assert!(document.analyzer(Uuid::nil()).is_err());
    }
    #[test]
    fn remove_analyzer() {
        let mut document = OpmDocument::default();
        let uuid1 = document.add_analyzer(AnalyzerType::Energy);
        let uuid2 = document.add_analyzer(AnalyzerType::Energy);

        assert!(document.remove_analyzer(uuid1).is_ok());
        assert_eq!(document.analyzers.len(), 1);
        assert!(document.remove_analyzer(Uuid::nil()).is_err());
        assert!(document.remove_analyzer(uuid2).is_ok());
        assert!(document.analyzers.is_empty());
    }
    #[test]
    fn all_nodes_integration_test() {
        let mut scenery = NodeGroup::default();
        let src = round_collimated_ray_source(millimeter!(10.0), joule!(1.0), 1).unwrap();
        let i_0 = scenery.add_node(src).unwrap();
        let i_1 = scenery.add_node(BeamSplitter::default()).unwrap();
        let i_2 = scenery.add_node(CylindricLens::default()).unwrap();
        let i_3 = scenery.add_node(FluenceDetector::default()).unwrap();
        let i_4 = scenery.add_node(Lens::default()).unwrap();
        let i_5 = scenery.add_node(Wedge::default()).unwrap();
        let i_6 = scenery.add_node(Dummy::default()).unwrap();
        let i_7 = scenery.add_node(EnergyMeter::default()).unwrap();
        let i_8 = scenery.add_node(IdealFilter::default()).unwrap();
        let i_9 = scenery
            .add_node(ParaxialSurface::new("paraxial", millimeter!(1000.0)).unwrap())
            .unwrap();
        let i_10 = scenery
            .add_node(RayPropagationVisualizer::default())
            .unwrap();
        let i_11 = scenery.add_node(Spectrometer::default()).unwrap();
        let i_12 = scenery.add_node(SpotDiagram::default()).unwrap();
        let i_13 = scenery.add_node(WaveFront::default()).unwrap();
        let i_14 = scenery.add_node(ParabolicMirror::default()).unwrap();
        let i_15 = scenery
            .add_node(
                ReflectiveGrating::default()
                    .with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))
                    .unwrap(),
            )
            .unwrap();
        let i_16 = scenery.add_node(ThinMirror::default()).unwrap();

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
    }
    #[test]
    fn full_analysis_with_save_and_load() {
        let mut scenery = NodeGroup::new("Lens Ray-trace test");
        let src = scenery
            .add_node(collimated_line_ray_source(millimeter!(20.0), joule!(1.0), 6).unwrap())
            .unwrap();
        let lens1 = Wedge::new(
            "Wedge",
            millimeter!(10.0),
            degree!(0.0),
            &RefrIndexConst::new(1.5068).unwrap(),
        )
        .unwrap()
        .with_tilt(degree!(15.0, 0.0, 0.0))
        .unwrap();
        let l1 = scenery.add_node(lens1).unwrap();
        let lens2 = Lens::new(
            "Lens 2",
            millimeter!(205.55),
            millimeter!(-205.55),
            millimeter!(2.79),
            &RefrIndexConst::new(1.5068).unwrap(),
        )
        .unwrap()
        .with_tilt(degree!(15.0, 0.0, 0.0))
        .unwrap();
        let l2 = scenery.add_node(lens2).unwrap();
        let det = scenery
            .add_node(RayPropagationVisualizer::new("Ray plot", None).unwrap())
            .unwrap();
        scenery
            .connect_nodes(src, "output_1", l1, "input_1", millimeter!(50.0))
            .unwrap();
        scenery
            .connect_nodes(l1, "output_1", l2, "input_1", millimeter!(50.0))
            .unwrap();
        scenery
            .connect_nodes(l2, "output_1", det, "input_1", millimeter!(50.0))
            .unwrap();
        let mut doc = OpmDocument::new(scenery);
        doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
        let temp_model_file = NamedTempFile::new().unwrap();
        doc.save_to_file(temp_model_file.path()).unwrap();

        testing_logger::setup();
        let mut doc = OpmDocument::from_file(temp_model_file.path()).unwrap();
        let _ = doc.analyze().unwrap();
        check_logs(log::Level::Warn, vec![]);
    }
}
