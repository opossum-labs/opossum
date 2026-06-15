#![warn(missing_docs)]
//! The basic structure of an OPOSSUM model.
//!
//! It contains the [`OpmDocument`] structure, which holds a (toplevel) [`NodeGroup`] representing the actual optical model
//! as well as a list of analyzers with their particular configuration and a global scene configuration (e.g. ambient medium etc.).
//!
//! This module also handles reading and writing of `.opm` files.
use crate::{
    analyzers::{Analyzer, AnalyzerRegistration, AnalyzerType},
    core_optics::{OpticNode, SceneryResources},
    error::{OpmResult, OpossumError},
    nodes::NodeGroup,
    reporting::analysis_report::AnalysisReport,
    utils::{
        LockExt,
        file_utils::{create_f_path, create_file_instance},
    },
};
use log::{info, warn};
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};
use utoipa::ToSchema;
use uuid::Uuid;

use ron::{extensions::Extensions, ser::PrettyConfig};

/// A structure containing the [`AnalyzerType`] together with its position on a frontend GUI.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct AnalyzerInfo {
    analyzer_type: AnalyzerType,
    gui_position: Option<(f64, f64)>,
}
impl AnalyzerInfo {
    /// Creates a new [`AnalyzerInfo`].
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(analyzer_type: AnalyzerType, gui_position: Point2<f64>) -> Self {
        Self {
            analyzer_type,
            gui_position: Some((gui_position.x, gui_position.y)),
        }
    }
    /// Returns the gui position of this [`AnalyzerInfo`].
    #[must_use]
    pub fn gui_position(&self) -> Option<Point2<f64>> {
        self.gui_position.map(|(x, y)| Point2::new(x, y))
    }
    /// Sets the gui position of this [`AnalyzerInfo`].
    pub fn set_gui_position(&mut self, gui_position: Option<Point2<f64>>) {
        self.gui_position = gui_position.map(|gp| (gp.x, gp.y));
    }
    /// Returns a reference to the analyzer type of this [`AnalyzerInfo`].
    #[must_use]
    pub const fn analyzer_type(&self) -> &AnalyzerType {
        &self.analyzer_type
    }
    /// Sets the analyzer type of this [`AnalyzerInfo`].
    pub fn set_analyzer_type(&mut self, analyzer_type: &AnalyzerType) {
        self.analyzer_type = analyzer_type.clone();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
/// The main structure of an OPOSSUM model.
/// It contains the [`NodeGroup`] representing the optical model, a list of analyzers and a global configuration.
pub struct OpmDocument {
    opm_file_version: String,
    #[serde(default)]
    scenery: NodeGroup,
    #[serde(default, rename = "global")]
    global_conf: Arc<Mutex<SceneryResources>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    analyzers: HashMap<Uuid, AnalyzerInfo>,
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
            warn!(
                "This file might haven been written by an older or newer version of OPOSSUM. The model import might not be correct."
            );
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
    /// Returns the content of the `.opm` file from this [`OpmDocument`]
    ///
    /// # Errors
    ///
    /// This function will return an error if the serialization of the internal structures fail.
    pub fn to_opm_file_string(&self) -> OpmResult<String> {
        let config = PrettyConfig::new()
            .extensions(Extensions::UNWRAP_VARIANT_NEWTYPES)
            .new_line("\n");
        ron::ser::to_string_pretty(&self, config).map_err(|e| {
            OpossumError::OpticScenery(format!("serialization of OpmDocument failed: {e}"))
        })
    }
    /// Returns the list of analyzers of this [`OpmDocument`].
    #[must_use]
    pub fn analyzers(&self) -> HashMap<Uuid, AnalyzerInfo> {
        self.analyzers.clone()
    }
    /// Returns a mutable reference of the analyzer with the given [`Uuid`] of this [`OpmDocument`].
    ///
    /// If an analyzer with the given [`Uuid`] is not found, `None` is returned.
    #[must_use]
    pub fn analyzer_mut(&mut self, id: Uuid) -> Option<&mut AnalyzerInfo> {
        self.analyzers.get_mut(&id)
    }
    /// Return an [`AnalyzerInfo`] with the given [`Uuid`] from this [`OpmDocument`].
    ///
    /// # Errors
    ///
    /// This functions returns an error if the [`AnalyzerInfo`] with the given [`Uuid`] was not found.
    pub fn analyzer(&self, id: Uuid) -> OpmResult<AnalyzerInfo> {
        self.analyzers.get(&id).map_or_else(
            || {
                Err(OpossumError::OpmDocument(
                    "Analyzer with given Uuid not found.".into(),
                ))
            },
            |analyzer_info| Ok(analyzer_info.clone()),
        )
    }
    /// Add an analyzer to this [`OpmDocument`].
    pub fn add_analyzer(&mut self, analyzer_type: AnalyzerType) -> Uuid {
        let id = Uuid::new_v4();
        let analyzer_info = AnalyzerInfo {
            analyzer_type,
            gui_position: None,
        };
        self.analyzers.insert(id, analyzer_info);
        id
    }
    /// Add an analyzer (with a GUI position) to this [`OpmDocument`].
    pub fn add_analyzer_with_position(
        &mut self,
        analyzer_type: AnalyzerType,
        gui_position: Option<(f64, f64)>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let analyzer_info = AnalyzerInfo {
            analyzer_type,
            gui_position,
        };
        self.analyzers.insert(id, analyzer_info);
        id
    }
    /// Add an analyzer to this [`OpmDocument`].
    // pub fn add_analyzer_info(&mut self, analyzer_info: &AnalyzerInfo) -> Uuid {
    //     self.analyzers
    //         .insert(analyzer_info.id, analyzer_info.clone());
    //     analyzer_info.id
    // }
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
    pub const fn scenery_mut(&mut self) -> &mut NodeGroup {
        &mut self.scenery
    }
    /// Returns a reference to the global config of this [`OpmDocument`].
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
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
            let analyzer_type = &ana.1.1.analyzer_type;
            let analyzer_box = inventory::iter::<AnalyzerRegistration>
                .into_iter()
                .find_map(|reg| (reg.builder)(analyzer_type))
                .ok_or_else(|| {
                    OpossumError::Other(format!(
                        "No analyzer implementation found for type: {analyzer_type:?}"
                    ))
                })?;
            let analyzer: &dyn Analyzer = &*analyzer_box;
            info!("Analysis #{}", ana.0);
            analyzer.analyze(&mut self.scenery)?;
            info!("Generating report #{}", ana.0);
            reports.push(analyzer.report(&self.scenery)?);
            self.scenery.clear_edges();
            self.scenery.reset_data();
        }
        Ok(reports)
    }
    /// Returns a mutable reference to the analyzers of this [`OpmDocument`].
    pub const fn analyzers_mut(&mut self) -> &mut HashMap<Uuid, AnalyzerInfo> {
        &mut self.analyzers
    }
    /// Create a DOT & SVG diagram file of optical model (scenery).
    ///
    /// This is a helper function being used in the CLI and the backend.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file creation fails.
    pub fn create_dot_file(&self, dot_path: &Path) -> OpmResult<()> {
        let mut output = create_file_instance(dot_path, "scenery", "dot")?;
        write!(output, "{}", self.scenery.toplevel_dot("")?)
            .map_err(|e| OpossumError::Other(format!("writing diagram file (.dot) failed: {e}")))?;
        let mut output = create_file_instance(dot_path, "scenery", "svg")?;
        let f_path = create_f_path(dot_path, "scenery", "dot");
        self.scenery.toplevel_dot_svg(&f_path, &mut output)
    }
    /// Checks if any node or analyzer in the document is missing its GUI coordinates.
    /// This is used to signal the frontend that an automatic layout is required.
    #[must_use]
    pub fn needs_autolayout(&self) -> bool {
        // 1. Check if any analyzer is missing its GUI position
        if self.analyzers.values().any(|a| a.gui_position().is_none()) {
            return true;
        }
        // 2. Check optical nodes
        for node_ref in self.scenery.nodes() {
            if let Ok(node) = node_ref.optical_ref.lock_opm()
                && node.gui_position().is_none()
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            Analyzer, GhostFocusConfig, RayTraceConfig, energy::EnergyConfig,
            ghostfocus::GhostFocusAnalyzer, raytrace::RayTracingAnalyzer,
        },
        core_optics::{Alignable, OpticNode},
        degree, joule, millimeter, nanometer,
        nodes::round_collimated_ray_builder,
        prelude::*,
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
    fn save_to_file() -> OpmResult<()> {
        let file = NamedTempFile::new()
            .map_err(|e| OpossumError::OpmDocument(format!("Error generating temp file: {e}")))?;
        let path = file.into_temp_path();
        let document = OpmDocument::default();
        assert!(document.save_to_file(&path).is_ok());
        path.close()
            .map_err(|e| OpossumError::OpmDocument(format!("Error closing temp file: {e}")))?;
        Ok(())
    }
    #[test]
    fn add_analyzer() {
        let mut document = OpmDocument::default();
        assert!(document.analyzers.is_empty());
        document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
        assert_eq!(document.analyzers.len(), 1);
    }
    #[test]
    fn add_analyzer_with_position() {
        let mut document = OpmDocument::default();
        let uuid = document
            .add_analyzer_with_position(AnalyzerType::Energy(EnergyConfig::default()), None);
        assert!(!uuid.is_nil());
    }
    #[test]
    fn analyzers() {
        let mut document = OpmDocument::default();
        document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
        document.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
        assert_eq!(document.analyzers().len(), 2);
    }
    #[test]
    fn analyzer() {
        let mut document = OpmDocument::default();
        let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
        let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

        assert!(document.analyzer(uuid1).is_ok());
        assert!(document.analyzer(uuid2).is_ok());
        assert!(document.analyzer(Uuid::nil()).is_err());
    }
    #[test]
    fn analyzer_mut() {
        let mut document = OpmDocument::default();
        let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
        let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

        assert!(document.analyzer_mut(uuid1).is_some());
        assert!(document.analyzer_mut(uuid2).is_some());
        assert!(document.analyzer_mut(Uuid::nil()).is_none());
    }
    #[test]
    fn remove_analyzer() {
        let mut document = OpmDocument::default();
        let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
        let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

        assert!(document.remove_analyzer(uuid1).is_ok());
        assert_eq!(document.analyzers.len(), 1);
        assert!(document.remove_analyzer(Uuid::nil()).is_err());
        assert!(document.remove_analyzer(uuid2).is_ok());
        assert!(document.analyzers.is_empty());
    }
    #[test]
    fn all_nodes_integration_test() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let i_0 = scenery.add_node(SourcePort::default())?;
        let i_1 = scenery.add_node(BeamSplitter::default())?;
        let i_2 = scenery.add_node(CylindricLens::default())?;
        let i_3 = scenery.add_node(FluenceDetector::default())?;
        let i_4 = scenery.add_node(Lens::default())?;
        let i_5 = scenery.add_node(Wedge::default())?;
        let i_6 = scenery.add_node(Dummy::default())?;
        let i_7 = scenery.add_node(EnergyMeter::default())?;
        let i_8 = scenery.add_node(IdealFilter::default())?;
        let i_9 = scenery.add_node(ParaxialSurface::new("paraxial", millimeter!(1000.0))?)?;
        let i_10 = scenery.add_node(RayPropagationVisualizer::default())?;
        let i_11 = scenery.add_node(Spectrometer::default())?;
        let i_12 = scenery.add_node(SpotDiagram::default())?;
        let i_13 = scenery.add_node(WaveFront::default())?;
        let i_14 = scenery.add_node(ParabolicMirror::default())?;
        let i_15 = scenery.add_node(
            ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))?,
        )?;
        let i_16 = scenery.add_node(ThinMirror::default())?;

        scenery.connect_nodes(i_0, "output_1", i_1, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_1, "out1_trans1_refl2", i_2, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_2, "output_1", i_3, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_3, "output_1", i_4, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_4, "output_1", i_5, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_5, "output_1", i_6, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_6, "output_1", i_7, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_7, "output_1", i_8, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_8, "output_1", i_9, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_9, "output_1", i_10, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_10, "output_1", i_11, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_11, "output_1", i_12, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_12, "output_1", i_13, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_13, "output_1", i_14, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_14, "output_1", i_15, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(i_15, "output_1", i_16, "input_1", millimeter!(50.0))?;

        scenery.set_global_conf(Some(Arc::new(Mutex::new(SceneryResources::default()))));
        let ray_builder = round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?;
        let mut config = RayTraceConfig::default();
        config.map_source(i_0, ray_builder.clone());
        // Perform ray tracing analysis
        testing_logger::setup();
        let analyzer = RayTracingAnalyzer::new(config);
        analyzer.analyze(&mut scenery)?;
        check_logs(log::Level::Warn, vec![]);
        scenery.reset_data();
        // Perform ghost focus analysis
        let mut config = GhostFocusConfig::default();
        config.map_source(i_0, ray_builder);
        let analyzer = GhostFocusAnalyzer::new(config);
        analyzer.analyze(&mut scenery)?;
        check_logs(log::Level::Warn, vec![]);
        Ok(())
    }
    #[test]
    fn full_analysis_with_save_and_load() -> OpmResult<()> {
        let mut scenery = NodeGroup::new("Lens Ray-trace test");
        let src = scenery.add_node(SourcePort::default())?;
        let lens1 = Wedge::new(
            "Wedge",
            millimeter!(10.0),
            degree!(0.0),
            &RefrIndexConst::new(1.5068)?,
        )?
        .with_tilt(degree!(15.0, 0.0, 0.0))?;
        let l1 = scenery.add_node(lens1)?;
        let lens2 = Lens::new(
            "Lens 2",
            millimeter!(205.55),
            millimeter!(-205.55),
            millimeter!(2.79),
            &RefrIndexConst::new(1.5068)?,
        )?
        .with_tilt(degree!(15.0, 0.0, 0.0))?;
        let l2 = scenery.add_node(lens2)?;
        let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot", None)?)?;
        scenery.connect_nodes(src, "output_1", l1, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(l1, "output_1", l2, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(l2, "output_1", det, "input_1", millimeter!(50.0))?;
        let mut doc = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            collimated_line_ray_builder(millimeter!(20.0), joule!(1.0), 6)?,
        );
        doc.add_analyzer(AnalyzerType::RayTrace(config));
        let temp_model_file = NamedTempFile::new()
            .map_err(|e| OpossumError::OpmDocument(format!("Error generating temp file: {e}")))?;
        doc.save_to_file(temp_model_file.path())?;

        testing_logger::setup();
        let mut doc = OpmDocument::from_file(temp_model_file.path())?;
        let _ = doc.analyze()?;
        check_logs(log::Level::Warn, vec![]);
        Ok(())
    }
    #[test]
    fn create_dot_file_test() -> OpmResult<()> {
        let document =
            OpmDocument::from_file(&Path::new("./files_for_testing/opm/opticscenery.opm"))?;
        assert!(
            document
                .create_dot_file(&Path::new("./files_for_testing/dot/_not_valid/"))
                .is_err()
        );
        assert!(
            document
                .create_dot_file(&Path::new("./files_for_testing/dot/"))
                .is_ok()
        );
        fs::remove_file("./files_for_testing/dot/scenery.dot")
            .map_err(|e| OpossumError::OpmDocument(format!("Error removing temp file: {e}")))?;
        fs::remove_file("./files_for_testing/dot/scenery.svg")
            .map_err(|e| OpossumError::OpmDocument(format!("Error generating temp file: {e}")))?;
        Ok(())
    }
    #[test]
    fn analyzer_info_set_analyzer_type() {
        let mut at = AnalyzerInfo::new(
            AnalyzerType::Energy(EnergyConfig::default()),
            Point2::new(1.0, 2.0),
        );
        at.set_analyzer_type(&AnalyzerType::GhostFocus(GhostFocusConfig::default()));
        assert!(matches!(at.analyzer_type, AnalyzerType::GhostFocus(_)));
    }
    #[test]
    fn analyzer_info_set_gui_position() {
        let mut at = AnalyzerInfo::new(
            AnalyzerType::Energy(EnergyConfig::default()),
            Point2::new(1.0, 2.0),
        );
        let new_position = Point2::new(3.0, 4.0);
        at.set_gui_position(Some(new_position));
        assert_eq!(at.gui_position(), Some(new_position))
    }
}
