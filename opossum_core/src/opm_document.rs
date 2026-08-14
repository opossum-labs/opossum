#![warn(missing_docs)]
//! The basic structure of an OPOSSUM model.
//!
//! It contains the [`OpmDocument`] structure, which holds a (toplevel) [`NodeGroup`] representing the actual optical model
//! as well as a list of analyzers with their particular configuration and a global scene configuration (e.g. ambient medium etc.).
//!
//! This module also handles reading and writing of `.opm` files.
use crate::{
    analyzers::{Analyzer, AnalyzerRegistration, AnalyzerType},
    core_optics::{NodeAttrExt, OpticNode, SceneryResources},
    error::{OpmResult, OpossumError},
    gain::PumpScenario,
    material::Material,
    nodes::NodeGroup,
    properties::{Proptype, proptype::AssetRef},
    reporting::analysis_report::AnalysisReport,
    utils::{
        LockExt,
        file_utils::{create_f_path, create_file_instance},
    },
};
use indexmap::IndexMap;
use log::{info, warn};
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use std::{
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
    #[serde(skip_serializing_if = "Option::is_none")]
    gui_position: Option<(f64, f64)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pump_scenarios: Vec<Uuid>,
}
impl AnalyzerInfo {
    /// Creates a new [`AnalyzerInfo`].
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn new(analyzer_type: AnalyzerType, gui_position: Point2<f64>) -> Self {
        Self {
            analyzer_type,
            gui_position: Some((gui_position.x, gui_position.y)),
            pump_scenarios: Vec::new(),
        }
    }
    /// Returns the [`PumpScenario`]s this analyzer is run in.
    ///
    /// An analyzer referring to no scenario at all is run once on the passive model, which is what
    /// every analyzer did before scenarios existed.
    #[must_use]
    pub fn pump_scenarios(&self) -> &[Uuid] {
        &self.pump_scenarios
    }
    /// Sets the [`PumpScenario`]s this analyzer is run in.
    ///
    /// The analyzer produces one report per listed scenario, in the given order.
    ///
    /// # Arguments
    ///
    /// * `pump_scenarios` - the scenarios to run, or an empty list for a purely passive run.
    pub fn set_pump_scenarios(&mut self, pump_scenarios: Vec<Uuid>) {
        self.pump_scenarios = pump_scenarios;
    }
    /// Stops running this analyzer in the [`PumpScenario`] with the given [`Uuid`].
    ///
    /// Called when that scenario is deleted: an analyzer must not keep pointing at an operating
    /// point that no longer exists.
    ///
    /// # Arguments
    ///
    /// * `id` - the scenario no longer to be run.
    fn remove_pump_scenario(&mut self, id: Uuid) {
        self.pump_scenarios.retain(|scenario_id| *scenario_id != id);
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
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    analyzers: IndexMap<Uuid, AnalyzerInfo>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pump_scenarios: IndexMap<Uuid, PumpScenario>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    embedded_materials: IndexMap<Uuid, Material>,
}
impl Default for OpmDocument {
    fn default() -> Self {
        Self {
            opm_file_version: env!("OPM_FILE_VERSION").to_string(),
            scenery: NodeGroup::default(),
            global_conf: Arc::new(Mutex::new(SceneryResources::default())),
            analyzers: IndexMap::default(),
            pump_scenarios: IndexMap::default(),
            embedded_materials: IndexMap::default(),
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
    /// Replaces all `AssetRef::Id(Uuid)` properties in scene nodes with
    /// the full `AssetRef::Inline(Material)` looked up from `embedded_materials`.
    fn resolve_embedded_materials(&self) -> OpmResult<()> {
        for node_ref in self.scenery.nodes() {
            if let Ok(mut node) = node_ref.optical_ref.lock_opm() {
                let mut updates = Vec::new();

                for (prop_name, prop) in node.node_attr().properties() {
                    if let Proptype::Material(AssetRef::Id(id)) = prop.prop() {
                        let material = self.embedded_materials.get(id).ok_or_else(|| {
                            OpossumError::OpmDocument(format!(
                                "Embedded material with UUID {id} not found for property '{prop_name}' in node '{}'",
                                node.node_attr().name()
                            ))
                        })?;

                        updates.push((
                            prop_name.clone(),
                            Proptype::Material(AssetRef::Inline(material.clone())),
                        ));
                    }
                }

                for (prop_name, new_prop) in updates {
                    node.node_attr_mut().set_property(&prop_name, new_prop)?;
                }
            }
        }
        Ok(())
    }

    /// Extracts full `Material` structs into `embedded_materials` and replaces
    /// node properties with explicit `AssetRef::Id(Uuid)`.
    fn prepare_materials_for_serialization(&mut self) -> OpmResult<()> {
        for node_ref in self.scenery.nodes() {
            if let Ok(mut node) = node_ref.optical_ref.lock_opm() {
                let mut updates = Vec::new();

                for (prop_name, prop) in node.node_attr().properties() {
                    if let Proptype::Material(AssetRef::Inline(material)) = prop.prop() {
                        self.embedded_materials
                            .insert(material.id(), material.clone());

                        updates.push((
                            prop_name.clone(),
                            Proptype::Material(AssetRef::Id(material.id())),
                        ));
                    }
                }

                for (prop_name, new_prop) in updates {
                    node.node_attr_mut().set_property(&prop_name, new_prop)?;
                }
            }
        }
        Ok(())
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
        // Resolve every reference against the now fully-built scenery, once at the root. A reference nested
        // in a group can point at a node in an ancestor or sibling branch that didn't exist yet during
        // per-group deserialization (see `OpticGraph::resolve_all_references`); this can't live in
        // `after_deserialization_hook`, which also runs per-node bottom-up during parse (before the whole
        // tree exists) via `OpticRef`'s deserializer.
        document.scenery.graph().resolve_all_references()?;

        // Resolve embedded material references into full in-memory Material structs
        document.resolve_embedded_materials()?;

        document
            .scenery
            .graph_mut()
            .update_global_config(&Some(document.global_conf.clone()));
        Ok(document)
    }
    /// Saves this [`OpmDocument`] to an `.opm` file at the specified path.
    ///
    /// This is a read-only operation on `&self` and does not mutate the in-memory document state.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if file creation, writing, or serialization fails.
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
    /// Generates the RON string content representation of this [`OpmDocument`].
    ///
    /// Internally clones the document to extract embedded materials and replace node
    /// material properties with UUID references without mutating the original `self`.
    ///
    /// # Errors
    /// Returns an [`OpossumError`] if serialization fails.
    pub fn to_opm_file_string(&self) -> OpmResult<String> {
        // Create a temporary mutable clone for serialization preparation
        let mut doc_to_serialize = self.clone();
        doc_to_serialize.prepare_materials_for_serialization()?;

        let config = PrettyConfig::new()
            .extensions(Extensions::UNWRAP_VARIANT_NEWTYPES)
            .new_line("\n");

        ron::ser::to_string_pretty(&doc_to_serialize, config).map_err(|e| {
            OpossumError::OpticScenery(format!("serialization of OpmDocument failed: {e}"))
        })
    }
    /// Returns the list of analyzers of this [`OpmDocument`].
    #[must_use]
    pub fn analyzers(&self) -> IndexMap<Uuid, AnalyzerInfo> {
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
            pump_scenarios: Vec::new(),
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
            pump_scenarios: Vec::new(),
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
        if self.analyzers.shift_remove(&id).is_some() {
            Ok(())
        } else {
            Err(OpossumError::OpmDocument(
                "Analyzer with given Uuid not found".into(),
            ))
        }
    }
    /// Re-inserts a previously-removed analyzer under its original [`Uuid`].
    ///
    /// Unlike [`add_analyzer_with_position`](Self::add_analyzer_with_position), this does not mint a new id -
    /// it is used to restore an analyzer to the exact identity it had before, which undo/redo relies on so that
    /// later history entries referencing that id (e.g. a config patch) keep resolving correctly.
    pub fn insert_analyzer(&mut self, id: Uuid, info: AnalyzerInfo) {
        self.analyzers.insert(id, info);
    }
    /// Return all [`PumpScenario`]s of this [`OpmDocument`].
    ///
    /// The scenarios are the operating points the model can be analyzed in. A document without any
    /// is a purely passive model, which is what every document starts out as.
    #[must_use]
    pub const fn pump_scenarios(&self) -> &IndexMap<Uuid, PumpScenario> {
        &self.pump_scenarios
    }
    /// Return the [`PumpScenario`] with the given [`Uuid`], if there is one.
    ///
    /// # Arguments
    ///
    /// * `id` - the scenario to look up.
    #[must_use]
    pub fn pump_scenario(&self, id: Uuid) -> Option<&PumpScenario> {
        self.pump_scenarios.get(&id)
    }
    /// Return a mutable reference to the [`PumpScenario`] with the given [`Uuid`], if there is one.
    ///
    /// This is how a single node is added to or removed from a scenario, without touching the model.
    ///
    /// # Arguments
    ///
    /// * `id` - the scenario to modify.
    pub fn pump_scenario_mut(&mut self, id: Uuid) -> Option<&mut PumpScenario> {
        self.pump_scenarios.get_mut(&id)
    }
    /// Add a new, empty [`PumpScenario`] with the given name to this [`OpmDocument`].
    ///
    /// # Arguments
    ///
    /// * `name` - the name of the new scenario.
    ///
    /// # Returns
    ///
    /// The [`Uuid`] the new scenario is addressed by.
    pub fn add_pump_scenario(&mut self, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.pump_scenarios.insert(id, PumpScenario::new(name));
        id
    }
    /// Re-insert a [`PumpScenario`] under a given [`Uuid`].
    ///
    /// Unlike [`add_pump_scenario`](Self::add_pump_scenario) this does not mint a new id, which is
    /// what restoring a removed scenario needs: anything else referring to that scenario keeps
    /// resolving. Same role as [`insert_analyzer`](Self::insert_analyzer).
    ///
    /// # Arguments
    ///
    /// * `id` - the identity the scenario is restored under.
    /// * `scenario` - the scenario to insert.
    pub fn insert_pump_scenario(&mut self, id: Uuid, scenario: PumpScenario) {
        self.pump_scenarios.insert(id, scenario);
    }
    /// Remove the [`PumpScenario`] with the given [`Uuid`] from this [`OpmDocument`].
    ///
    /// Every analyzer running in that scenario stops doing so, since an operating point that no
    /// longer exists cannot be analyzed. An analyzer left without any scenario runs on the passive
    /// model again.
    ///
    /// # Arguments
    ///
    /// * `id` - the scenario to remove.
    ///
    /// # Returns
    ///
    /// The removed scenario, or `None` if there was none with that id.
    pub fn remove_pump_scenario(&mut self, id: Uuid) -> Option<PumpScenario> {
        let removed = self.pump_scenarios.shift_remove(&id)?;
        for analyzer in self.analyzers.values_mut() {
            analyzer.remove_pump_scenario(id);
        }
        Some(removed)
    }
    /// Drop the entries of deleted nodes from every [`PumpScenario`] of this [`OpmDocument`].
    ///
    /// Scenarios refer to nodes by [`Uuid`] and live beside the model rather than inside it, so
    /// deleting a node leaves them holding an entry that belongs to nothing. Running this after a
    /// deletion keeps the operating points consistent with the model they describe.
    pub fn prune_pump_scenarios(&mut self) {
        for scenario in self.pump_scenarios.values_mut() {
            scenario.prune(&self.scenery);
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
    /// An analyzer that refers to [`PumpScenario`]s is run once per scenario, so it contributes one
    /// report per operating point; one that refers to none is run once on the passive model.
    /// The results of the analysis will be returned as a vector of [`AnalysisReport`]s.
    ///
    /// # Errors
    ///
    /// This function will return an error if an analyzer refers to a [`PumpScenario`] that does not
    /// exist, or if the individual analyzers fail to perform the analysis.
    pub fn analyze(&mut self) -> OpmResult<Vec<AnalysisReport>> {
        if self.analyzers.is_empty() {
            info!("No analyzer defined in document. Stopping here.");
            return Ok(vec![]);
        }
        let runs = self.analysis_runs()?;
        let mut reports = vec![];
        for (analyzer_nr, analyzer_type, scenario_name) in runs {
            let analyzer_box = inventory::iter::<AnalyzerRegistration>
                .into_iter()
                .find_map(|reg| (reg.builder)(&analyzer_type))
                .ok_or_else(|| {
                    OpossumError::Other(format!(
                        "No analyzer implementation found for type: {analyzer_type:?}"
                    ))
                })?;
            let analyzer: &dyn Analyzer = &*analyzer_box;
            match &scenario_name {
                Some(name) => info!("Analysis #{analyzer_nr}, pump scenario '{name}'"),
                None => info!("Analysis #{analyzer_nr}"),
            }
            analyzer.analyze(&mut self.scenery)?;
            info!("Generating report #{analyzer_nr}");
            let mut report = analyzer.report(&self.scenery)?;
            if let Some(name) = &scenario_name {
                // The operating point belongs on the report the same way the kind of analysis does:
                // it is what distinguishes two otherwise identical reports of the same model.
                report.set_analysis_type(&format!("{} - {name}", report.analysis_type()));
            }
            reports.push(report);
            // Every run starts from the same state, so this has to happen between two scenarios of
            // one analyzer just as much as between two analyzers.
            self.scenery.clear_edges();
            self.scenery.reset_data();
        }
        Ok(reports)
    }
    /// Expand the analyzers of this [`OpmDocument`] into the individual runs to be performed.
    ///
    /// An analyzer contributes one run per [`PumpScenario`] it refers to, or a single passive run if
    /// it refers to none. Resolving the scenarios here rather than while analyzing means a reference
    /// to a scenario that does not exist is reported *before* the first ray is traced, instead of
    /// after a long analysis has already run.
    ///
    /// # Returns
    ///
    /// One entry per run: the number of the analyzer it belongs to, the analyzer to build, and the
    /// name of the operating point it runs in (if any). The entries are owned, so the document is
    /// free to be analyzed while the plan is walked.
    ///
    /// # Errors
    ///
    /// This function returns an error if an analyzer refers to a [`PumpScenario`] that does not
    /// exist in this document.
    fn analysis_runs(&self) -> OpmResult<Vec<(usize, AnalyzerType, Option<String>)>> {
        let mut runs = Vec::new();
        for (analyzer_nr, (_, analyzer_info)) in self.analyzers.iter().enumerate() {
            if analyzer_info.pump_scenarios.is_empty() {
                runs.push((analyzer_nr, analyzer_info.analyzer_type.clone(), None));
                continue;
            }
            for scenario_id in &analyzer_info.pump_scenarios {
                let scenario = self.pump_scenarios.get(scenario_id).ok_or_else(|| {
                    OpossumError::OpmDocument(format!(
                        "analysis #{analyzer_nr} refers to the pump scenario {scenario_id}, \
                         which does not exist"
                    ))
                })?;
                // The operating point rides along in the analyzer's own configuration, which is
                // what reaches the components during the run.
                let mut analyzer_type = analyzer_info.analyzer_type.clone();
                analyzer_type.set_pump_scenario(Some(scenario.clone()));
                runs.push((
                    analyzer_nr,
                    analyzer_type,
                    Some(scenario.name().to_string()),
                ));
            }
        }
        Ok(runs)
    }
    /// Returns a mutable reference to the analyzers of this [`OpmDocument`].
    pub const fn analyzers_mut(&mut self) -> &mut IndexMap<Uuid, AnalyzerInfo> {
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
        core_optics::{Alignable, OpticNode, node_attr::HasNodeAttr},
        degree,
        gain::{ConstGain, GainModel},
        joule,
        material::MATERIAL,
        millimeter, nanometer,
        nodes::round_collimated_ray_builder,
        prelude::*,
        refractive_index::RefrIndexConst,
        utils::test_helper::test_helper::check_logs,
    };
    use approx::assert_relative_eq;
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
        assert!(document.pump_scenarios.is_empty());
    }
    #[test]
    fn pump_scenario_crud() {
        let mut document = OpmDocument::default();
        let id = document.add_pump_scenario("full power");
        assert_eq!(document.pump_scenarios().len(), 1);
        assert_eq!(
            document.pump_scenario(id).map(PumpScenario::name),
            Some("full power")
        );
        assert!(document.pump_scenario(Uuid::new_v4()).is_none());

        document
            .pump_scenario_mut(id)
            .expect("the scenario just added must be there")
            .set_name("half power");
        assert_eq!(
            document.pump_scenario(id).map(PumpScenario::name),
            Some("half power")
        );

        let removed = document.remove_pump_scenario(id);
        assert_eq!(removed.as_ref().map(PumpScenario::name), Some("half power"));
        assert!(document.pump_scenarios().is_empty());
        assert!(document.remove_pump_scenario(id).is_none());

        // Restoring a scenario keeps its identity, so anything referring to it still resolves.
        document.insert_pump_scenario(id, removed.expect("the scenario was removed above"));
        assert_eq!(
            document.pump_scenario(id).map(PumpScenario::name),
            Some("half power")
        );
    }
    #[test]
    fn pruning_follows_deleted_nodes_in_every_scenario() -> OpmResult<()> {
        let mut document = OpmDocument::default();
        let lens_id = document.scenery_mut().add_node(Lens::default())?;
        let deleted_id = document.scenery_mut().add_node(Lens::default())?;
        let gain = GainModel::Const(ConstGain::new(2.0)?);
        for name in ["full power", "half power"] {
            let scenario_id = document.add_pump_scenario(name);
            let scenario = document
                .pump_scenario_mut(scenario_id)
                .expect("the scenario just added must be there");
            scenario.set_gain_model(lens_id, gain);
            scenario.set_gain_model(deleted_id, gain);
        }
        document.scenery_mut().delete_node(deleted_id)?;
        document.prune_pump_scenarios();

        for scenario in document.pump_scenarios().values() {
            assert_eq!(scenario.gain_model(lens_id), gain);
            assert_eq!(scenario.gain_model(deleted_id), GainModel::None);
        }
        Ok(())
    }
    /// A document carries its operating points, so they have to survive the way to a file and back.
    #[test]
    fn pump_scenarios_survive_a_file_round_trip() -> OpmResult<()> {
        let mut document = OpmDocument::default();
        let lens_id = document.scenery_mut().add_node(Lens::default())?;
        let scenario_id = document.add_pump_scenario("full power");
        let gain = GainModel::Const(ConstGain::new(2.0)?);
        document
            .pump_scenario_mut(scenario_id)
            .expect("the scenario just added must be there")
            .set_gain_model(lens_id, gain);

        let serialized = document.to_opm_file_string()?;
        let reloaded = OpmDocument::from_string(&serialized)?;
        assert_eq!(
            reloaded
                .pump_scenario(scenario_id)
                .map(|scenario| scenario.gain_model(lens_id)),
            Some(gain)
        );
        Ok(())
    }
    /// A passive document must not gain a `pump_scenarios` entry it never asked for.
    #[test]
    fn a_document_without_scenarios_writes_none() -> OpmResult<()> {
        let document = OpmDocument::default();
        assert!(!document.to_opm_file_string()?.contains("pump_scenarios"));
        Ok(())
    }
    /// A document that can be analyzed: one source feeding one energy meter, plus one analyzer.
    ///
    /// # Returns
    ///
    /// The document and the [`Uuid`] of its analyzer.
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be assembled.
    fn document_with_one_analyzer() -> OpmResult<(OpmDocument, Uuid)> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let meter = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(source, "output_1", meter, "input_1", millimeter!(10.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = EnergyConfig::default();
        config.map_source(
            source,
            EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
                vec![(nanometer!(1053.0), joule!(1.0))],
                nanometer!(1.0),
            )?),
        );
        let analyzer_id = document.add_analyzer(AnalyzerType::Energy(config));
        Ok((document, analyzer_id))
    }
    /// Analyze a document and return the analysis type of every report it produced.
    ///
    /// # Errors
    ///
    /// Returns an error if the analysis fails.
    fn analysis_types_of(document: &mut OpmDocument) -> OpmResult<Vec<String>> {
        Ok(document
            .analyze()?
            .iter()
            .map(|report| report.analysis_type().to_string())
            .collect())
    }
    /// Without an operating point nothing changes: one analyzer, one report, same title as ever.
    #[test]
    fn an_analyzer_without_scenarios_is_run_once() -> OpmResult<()> {
        let (mut document, _) = document_with_one_analyzer()?;
        assert_eq!(analysis_types_of(&mut document)?, vec!["Energy Analysis"]);
        Ok(())
    }
    /// The point of scenarios: one model, several operating points, one report each.
    #[test]
    fn an_analyzer_is_run_once_per_scenario() -> OpmResult<()> {
        let (mut document, analyzer_id) = document_with_one_analyzer()?;
        let full_power = document.add_pump_scenario("full power");
        let half_power = document.add_pump_scenario("half power");
        document
            .analyzer_mut(analyzer_id)
            .expect("the analyzer just added must be there")
            .set_pump_scenarios(vec![full_power, half_power]);
        assert_eq!(
            analysis_types_of(&mut document)?,
            vec![
                "Energy Analysis - full power",
                "Energy Analysis - half power"
            ]
        );
        Ok(())
    }
    /// A scenario that is gone must not be mistaken for "no scenario": that would silently report a
    /// passive run under the name of an operating point nobody defined.
    #[test]
    fn an_analyzer_pointing_at_a_missing_scenario_is_an_error() -> OpmResult<()> {
        let (mut document, analyzer_id) = document_with_one_analyzer()?;
        let missing_id = Uuid::new_v4();
        document
            .analyzer_mut(analyzer_id)
            .expect("the analyzer just added must be there")
            .set_pump_scenarios(vec![missing_id]);
        let message = document.analyze().unwrap_err().to_string();
        assert!(
            message.contains(&missing_id.to_string()),
            "the error has to name the missing scenario, got: {message}"
        );
        Ok(())
    }
    /// Read the energy an [`EnergyMeter`] recorded from an analysis report.
    ///
    /// # Errors
    ///
    /// Returns an error if the report contains no energy reading at all.
    fn metered_energy(report: &AnalysisReport) -> OpmResult<f64> {
        report
            .node_reports()
            .iter()
            .find_map(|node_report| match node_report.properties().get("Energy") {
                Ok(Proptype::Energy(energy)) => Some(energy.value),
                _ => None,
            })
            .ok_or_else(|| OpossumError::Other("no energy reading in the report".into()))
    }
    /// The whole point of scenarios: the same model, analyzed in two operating points, gives two
    /// different results - here a lens amplifying twice as strongly in one of them.
    #[test]
    fn two_scenarios_give_two_different_results() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let lens = scenery.add_node(Lens::default())?;
        let meter = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(source, "output_1", lens, "input_1", millimeter!(10.0))?;
        scenery.connect_nodes(lens, "output_1", meter, "input_1", millimeter!(10.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = EnergyConfig::default();
        config.map_source(
            source,
            EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
                vec![(nanometer!(1053.0), joule!(1.0))],
                nanometer!(1.0),
            )?),
        );
        let analyzer_id = document.add_analyzer(AnalyzerType::Energy(config));

        let mut scenario_ids = Vec::new();
        for (name, gain) in [("full power", 4.0), ("half power", 2.0)] {
            let scenario_id = document.add_pump_scenario(name);
            document
                .pump_scenario_mut(scenario_id)
                .expect("the scenario just added must be there")
                .set_gain_model(lens, GainModel::Const(ConstGain::new(gain)?));
            scenario_ids.push(scenario_id);
        }
        document
            .analyzer_mut(analyzer_id)
            .expect("the analyzer just added must be there")
            .set_pump_scenarios(scenario_ids);

        let reports = document.analyze()?;
        assert_eq!(reports.len(), 2);
        let full_power = metered_energy(&reports[0])?;
        let half_power = metered_energy(&reports[1])?;
        assert_relative_eq!(full_power / half_power, 2.0, epsilon = 1e-12);
        Ok(())
    }
    /// Deleting an operating point must not leave an analyzer pointing at it.
    #[test]
    fn removing_a_scenario_stops_the_analyzers_running_it() -> OpmResult<()> {
        let (mut document, analyzer_id) = document_with_one_analyzer()?;
        let full_power = document.add_pump_scenario("full power");
        let half_power = document.add_pump_scenario("half power");
        document
            .analyzer_mut(analyzer_id)
            .expect("the analyzer just added must be there")
            .set_pump_scenarios(vec![full_power, half_power]);

        assert!(document.remove_pump_scenario(full_power).is_some());
        assert_eq!(
            document.analyzer(analyzer_id)?.pump_scenarios(),
            vec![half_power]
        );
        assert_eq!(
            analysis_types_of(&mut document)?,
            vec!["Energy Analysis - half power"]
        );
        Ok(())
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
    /// Regression test for a reference whose target lives one level *up* (in an ancestor group) failing to
    /// reload: `root { A, G { ref -> A } }`. Reference resolution used to run per-group during
    /// deserialization, which builds inner groups before outer ones - so `G`'s reference couldn't see `A`
    /// (in the not-yet-built root) and the load errored with "reference node found, which does not reference
    /// anything". It is now deferred to a whole-scenery pass run after the full tree exists. Round-trips the
    /// document through its `.opm` string and asserts it reloads with the reference resolving to A.
    #[test]
    fn reference_into_ancestor_round_trips() {
        use crate::{
            nodes::{Dummy, NodeReference},
            utils::LockExt,
        };

        let mut document = OpmDocument::default();
        let r_id = {
            let scenery = document.scenery_mut();
            let a_id = scenery.add_node(Dummy::default()).unwrap();
            let a_ref = scenery.node_recursive(a_id).unwrap().0;
            let mut g = NodeGroup::new("G");
            let r_id = g
                .add_node(NodeReference::from_node(&a_ref).unwrap())
                .unwrap();
            scenery.add_node(g).unwrap();
            r_id
        };

        let serialized = document.to_opm_file_string().unwrap();
        // Before the fix this errored: G's reference to A (a level up) couldn't resolve mid-deserialization.
        let reloaded = OpmDocument::from_string(&serialized)
            .expect("a reference pointing at an ancestor node must reload");

        let (reference, _) = reloaded
            .scenery()
            .node_recursive(r_id)
            .expect("the reference must still exist after reload");
        let ports = reference.optical_ref.lock_opm().unwrap().ports();
        assert!(
            !ports.names(&PortType::Output).is_empty(),
            "the reloaded reference must resolve to A (non-empty mirrored ports)"
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
    fn test_material_referencing_serialization_roundtrip() -> OpmResult<()> {
        // Note: Ensure AssetRef is imported in the test module:
        // use crate::properties::proptype::AssetRef;

        let material_id = Uuid::new_v4();
        let const_refr = RefrIndexConst::new(1.5)?;
        let material = Material::new_for_test(material_id, 1, "N-BK7 Shared", const_refr.into());

        let mut scenery = NodeGroup::default();

        // Create two lenses sharing the same material instance
        let lens1 = Lens::new(
            "Lens 1",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            material.clone(),
        )?;
        let lens2 = Lens::new(
            "Lens 2",
            millimeter!(200.0),
            millimeter!(-200.0),
            millimeter!(12.0),
            material,
        )?;

        scenery.add_node(lens1)?;
        scenery.add_node(lens2)?;

        let doc = OpmDocument::new(scenery);

        // Serialize to RON string
        let ron_str = doc.to_opm_file_string()?;

        // Verify RON contains embedded_materials table with the single material
        assert!(ron_str.contains("embedded_materials:"));
        assert!(ron_str.contains("N-BK7 Shared"));

        // Verify nodes in RON string use AssetRef::Id instead of duplicating full struct
        assert!(ron_str.contains("Id("));

        // Deserialize back from RON string
        let reloaded_doc = OpmDocument::from_string(&ron_str)?;

        // Verify that embedded_materials contains exactly 1 deduplicated entry
        assert_eq!(reloaded_doc.embedded_materials.len(), 1);

        // Verify that nodes have their full Material struct restored for calculation
        for node_ref in reloaded_doc.scenery().nodes() {
            let node = node_ref.optical_ref.lock_opm()?;
            let prop = node.node_attr().get_property("material")?;

            // Unpack the AssetRef::Inline to verify the material is correctly loaded into RAM
            if let Proptype::Material(AssetRef::Inline(mat)) = prop {
                assert_eq!(mat.id(), material_id);
                assert_eq!(mat.name(), "N-BK7 Shared");
            } else {
                panic!(
                    "Expected Proptype::Material(AssetRef::Inline) in node after deserialization resolution"
                );
            }
        }

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
    fn test_skip_unknown_node_type_during_deserialization() -> OpmResult<()> {
        // Raw RON model data with a valid dummy node, an unknown node type, and a second valid dummy node
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "84d4007c-514e-44ca-8b63-bfa86bee265f",
        "graph": (
            nodes: [
                {
                    "node_type": "dummy",
                    "name": "valid_dummy_1",
                    "uuid": "26e56527-c7f9-4e9c-9cda-0e0fa96e39bd",
                },
                {
                    "node_type": "unknown_future_node",
                    "name": "invalid_node",
                    "uuid": "c52398ba-1742-4d86-82e1-8e75874d91ba",
                },
                {
                    "node_type": "dummy",
                    "name": "valid_dummy_2",
                    "uuid": "54e2d453-9632-4b9d-b9c7-48491526f198",
                },
            ],
            edges: [],
        ),
    },
    global: (
        ambient_refr_index: Const(
            refractive_index: 1.0,
        ),
    ),
)"#;

        // Setup testing logger to catch emitted warnings
        testing_logger::setup();

        // Parse document from string
        let doc = OpmDocument::from_string(ron_data)?;

        // Ensure the unknown node was skipped and only 2 valid nodes remain
        assert_eq!(
            doc.scenery().nodes().len(),
            2,
            "Document graph should contain exactly 2 valid nodes after skipping the unknown node"
        );

        // Verify the exact warning string logged by OpticRef::deserialize
        check_logs(
            log::Level::Warn,
            vec![
                "Unknown node type 'unknown_future_node'. Skipping node: Opossum Error:Other:cannot create node type <unknown_future_node>",
            ],
        );

        Ok(())
    }
    /// Regression test for the `refractive index` -> `material` property rename.
    ///
    /// A node is rebuilt from its default and then updated with the properties read from the file.
    /// Since `Properties::update` silently skips keys the default node does not know, an `.opm`
    /// written before the rename would come back with the *default* material (n = 1.5) instead of
    /// the index it was saved with — a data loss without any error message. Without the migration
    /// hook in `Properties::deserialize` this test fails on the very first assertion.
    #[test]
    fn legacy_refractive_index_is_migrated_to_material() -> OpmResult<()> {
        // An `.opm` as written by OPOSSUM <= 0.7.2: a lens with a bare `refractive index` property
        // whose value (2.0) differs from the lens default (1.5).
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "6f0d3b1c-3c1a-4c8e-9b3e-1f2a3b4c5d6e",
        "graph": (
            nodes: [
                {
                    "node_type": "lens",
                    "name": "old lens",
                    "uuid": "1a2b3c4d-5e6f-4a8b-9c0d-1e2f3a4b5c6d",
                    "props": {
                        "refractive index": RefractiveIndex(Const(refractive_index: 2.0)),
                    },
                },
            ],
            edges: [],
        ),
    },
    global: (
        ambient_refr_index: Const(
            refractive_index: 1.0,
        ),
    ),
)"#;
        let lens_id = Uuid::parse_str("1a2b3c4d-5e6f-4a8b-9c0d-1e2f3a4b5c6d")
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        let refractive_index_of = |document: &OpmDocument| -> OpmResult<f64> {
            let (lens, _) = document.scenery().node_recursive(lens_id)?;
            // The lock is released at the end of this statement, the material is owned from here on.
            let property = lens
                .optical_ref
                .lock_opm()?
                .node_attr()
                .get_property(MATERIAL)
                .cloned();
            let Ok(Proptype::Material(AssetRef::Inline(material))) = property else {
                return Err(OpossumError::Other(
                    "lens has no embedded material property".into(),
                ));
            };
            material.get_refractive_index(nanometer!(1000.0))
        };

        // The index of the pre-rename file must survive the load instead of falling back to the
        // lens default of 1.5.
        let document = OpmDocument::from_string(ron_data)?;
        assert_relative_eq!(refractive_index_of(&document)?, 2.0);

        // Saving moves the migrated material into `embedded_materials` and leaves an `AssetRef::Id`
        // behind; loading hydrates it again. The value has to survive that detour as well.
        let reloaded = OpmDocument::from_string(&document.to_opm_file_string()?)?;
        assert_relative_eq!(refractive_index_of(&reloaded)?, 2.0);
        Ok(())
    }
    #[test]
    fn test_skip_invalid_edge_connection() -> OpmResult<()> {
        // Raw RON model data with one valid node and an edge pointing to a non-existent target UUID
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "0e0e825e-5f4c-4e0a-b6ce-6c25b608f4aa",
        "graph": (
            nodes: [
                {
                    "node_type": "dummy",
                    "name": "dummy_1",
                    "uuid": "ecb719a2-2e21-44d0-b0b9-1e4a813e964e",
                },
            ],
            edges: [
                (
                    src_id: "ecb719a2-2e21-44d0-b0b9-1e4a813e964e",
                    src_port: "output_1",
                    target_id: "00000000-0000-0000-0000-000000000000",
                    target_port: "input_1",
                    distance: 0.0,
                ),
            ],
        ),
    },
    global: (
        ambient_refr_index: Const(
            refractive_index: 1.0,
        ),
    ),
)"#;

        // Initialize testing logger to capture warnings during graph reconstruction
        testing_logger::setup();

        // Deserialization should succeed despite the broken connection
        let doc = OpmDocument::from_string(ron_data)?;

        // Verify that the valid node was loaded correctly
        assert_eq!(
            doc.scenery().nodes().len(),
            1,
            "The valid node should be present in the graph"
        );

        // Verify that the broken edge warning was recorded in the log with the exact error string
        check_logs(
            log::Level::Warn,
            vec![
                "Skipping invalid node connection from 'ecb719a2-2e21-44d0-b0b9-1e4a813e964e' (output_1) to '00000000-0000-0000-0000-000000000000' (input_1): OpticScenery:target node with given id does not exist",
            ],
        );

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
