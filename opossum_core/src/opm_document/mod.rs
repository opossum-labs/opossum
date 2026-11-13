#![warn(missing_docs)]
//! The basic structure of an OPOSSUM model.
//!
//! It contains the [`OpmDocument`] structure, which holds a (toplevel) [`NodeGroup`] representing the actual optical model
//! as well as a list of analyzers with their particular configuration and a global scene configuration (e.g. ambient medium etc.).
//!
//! This module also orchestrates model analysis and handles reading and writing of `.opm` files.
mod analysis;
mod analyzer_info;
mod io;
pub mod optimization;
#[cfg(test)]
mod tests;
use crate::{
    analyzers::AnalyzerType,
    error::{OpmResult, OpossumError},
    gain::{GainModel, PumpScenario},
    material::Material,
    nodes::NodeGroup,
};
pub use analyzer_info::AnalyzerInfo;
use indexmap::IndexMap;
pub use optimization::OptimizationConfig;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
/// Current version of the `.opm` file format schema.
pub const CURRENT_OPM_FILE_VERSION: u32 = 0;
/// Key identifying a cached scalar detector metric: `(analyzer_id, node_id, metric_name)`.
pub type MetricStoreKey = (Uuid, Uuid, String);
/// Map for storing analyzer-specific scalar detector metrics.
pub type MetricStore = HashMap<MetricStoreKey, f64>;
/// Map for storing analyzer-specific vector detector metrics (e.g., per-ray OPDs).
pub type VectorMetricStore = HashMap<MetricStoreKey, Vec<f64>>;
/// The main structure of an OPOSSUM model.
///
/// It contains the [`NodeGroup`] representing the optical model, a list of analyzers,
/// optional pump scenarios, embedded materials, and an optional optimization configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpmDocument {
    pub(crate) opm_file_version: u32,
    #[serde(default)]
    pub(crate) scenery: NodeGroup,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub(crate) analyzers: IndexMap<Uuid, AnalyzerInfo>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub(crate) pump_scenarios: IndexMap<Uuid, PumpScenario>,
    /// Nodes marked as amplifier candidates, independent of any [`PumpScenario`].
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub(crate) amplifier_nodes: HashSet<Uuid>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub(crate) embedded_materials: IndexMap<Uuid, Material>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) optimization_config: Option<OptimizationConfig>,
    #[serde(skip)]
    metric_store: MetricStore,
    #[serde(skip)]
    vector_metric_store: VectorMetricStore,
}
impl Default for OpmDocument {
    fn default() -> Self {
        Self {
            opm_file_version: CURRENT_OPM_FILE_VERSION,
            scenery: NodeGroup::default(),
            analyzers: IndexMap::default(),
            pump_scenarios: IndexMap::default(),
            amplifier_nodes: HashSet::default(),
            embedded_materials: IndexMap::default(),
            optimization_config: None,
            metric_store: HashMap::new(),
            vector_metric_store: HashMap::new(),
        }
    }
}
impl OpmDocument {
    /// Creates a new [`OpmDocument`] from the given root [`NodeGroup`].
    #[must_use]
    pub fn new(scenery: NodeGroup) -> Self {
        Self {
            scenery,
            ..Default::default()
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
    /// Returns a cloned map of all analyzers registered in this [`OpmDocument`].
    #[must_use]
    pub fn analyzers(&self) -> IndexMap<Uuid, AnalyzerInfo> {
        self.analyzers.clone()
    }
    /// Returns a mutable reference to the analyzers map of this [`OpmDocument`].
    pub const fn analyzers_mut(&mut self) -> &mut IndexMap<Uuid, AnalyzerInfo> {
        &mut self.analyzers
    }
    /// Returns a mutable reference to the analyzer with the given [`Uuid`].
    ///
    /// If an analyzer with the given [`Uuid`] is not found, `None` is returned.
    #[must_use]
    pub fn analyzer_mut(&mut self, id: Uuid) -> Option<&mut AnalyzerInfo> {
        self.analyzers.get_mut(&id)
    }
    /// Returns a cloned [`AnalyzerInfo`] with the given [`Uuid`] from this [`OpmDocument`].
    ///
    /// # Errors
    ///
    /// Returns an error if no [`AnalyzerInfo`] with the given [`Uuid`] exists.
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
    /// Adds an analyzer without a GUI position to this [`OpmDocument`] and returns its new [`Uuid`].
    pub fn add_analyzer(&mut self, analyzer_type: AnalyzerType) -> Uuid {
        let id = Uuid::new_v4();
        let analyzer_info = AnalyzerInfo::new_without_position(analyzer_type);
        self.analyzers.insert(id, analyzer_info);
        id
    }
    /// Adds an analyzer with an optional GUI position to this [`OpmDocument`] and returns its new [`Uuid`].
    pub fn add_analyzer_with_position(
        &mut self,
        analyzer_type: AnalyzerType,
        gui_position: Option<(f64, f64)>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let mut analyzer_info = AnalyzerInfo::new_without_position(analyzer_type);
        if let Some((x, y)) = gui_position {
            analyzer_info.set_gui_position_tuple(Some((x, y)));
        }
        self.analyzers.insert(id, analyzer_info);
        id
    }
    /// Removes the analyzer with the given [`Uuid`] from this [`OpmDocument`].
    ///
    /// # Errors
    ///
    /// Returns an error if no analyzer with the given [`Uuid`] was found.
    pub fn remove_analyzer(&mut self, id: Uuid) -> OpmResult<()> {
        if self.analyzers.shift_remove(&id).is_some() {
            Ok(())
        } else {
            Err(OpossumError::OpmDocument(
                "Analyzer with given Uuid not found".into(),
            ))
        }
    }
    /// Re-inserts a previously removed analyzer under its original [`Uuid`].
    ///
    /// Unlike [`add_analyzer_with_position`](Self::add_analyzer_with_position), this does not mint a new ID—
    /// it restores an analyzer to the exact identity it had before (e.g., for undo/redo).
    pub fn insert_analyzer(&mut self, id: Uuid, info: AnalyzerInfo) {
        self.analyzers.insert(id, info);
    }
    /// Returns all [`PumpScenario`]s of this [`OpmDocument`].
    #[must_use]
    pub const fn pump_scenarios(&self) -> &IndexMap<Uuid, PumpScenario> {
        &self.pump_scenarios
    }
    /// Returns a reference to the [`PumpScenario`] with the given [`Uuid`], if present.
    #[must_use]
    pub fn pump_scenario(&self, id: Uuid) -> Option<&PumpScenario> {
        self.pump_scenarios.get(&id)
    }
    /// Returns a mutable reference to the [`PumpScenario`] with the given [`Uuid`], if present.
    pub fn pump_scenario_mut(&mut self, id: Uuid) -> Option<&mut PumpScenario> {
        self.pump_scenarios.get_mut(&id)
    }
    /// Adds a new, empty [`PumpScenario`] with the given name and returns its newly generated [`Uuid`].
    pub fn add_pump_scenario(&mut self, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        self.pump_scenarios.insert(id, PumpScenario::new(name));
        id
    }
    /// Re-inserts a [`PumpScenario`] under a given [`Uuid`].
    pub fn insert_pump_scenario(&mut self, id: Uuid, scenario: PumpScenario) {
        self.pump_scenarios.insert(id, scenario);
    }
    /// Removes the [`PumpScenario`] with the given [`Uuid`] from this [`OpmDocument`].
    ///
    /// Every analyzer configured to run in that scenario automatically stops referencing it.
    pub fn remove_pump_scenario(&mut self, id: Uuid) -> Option<PumpScenario> {
        let removed = self.pump_scenarios.shift_remove(&id)?;
        for analyzer in self.analyzers.values_mut() {
            analyzer.remove_pump_scenario(id);
        }
        Some(removed)
    }
    /// Drops entries referencing deleted nodes from every [`PumpScenario`] in this [`OpmDocument`].
    pub fn prune_pump_scenarios(&mut self) {
        for scenario in self.pump_scenarios.values_mut() {
            scenario.prune(&self.scenery);
        }
    }
    /// Returns the set of node [`Uuid`]s marked as amplifier candidates in this [`OpmDocument`].
    #[must_use]
    pub const fn amplifier_nodes(&self) -> &HashSet<Uuid> {
        &self.amplifier_nodes
    }
    /// Returns whether the node with the given [`Uuid`] is marked as an amplifier candidate.
    #[must_use]
    pub fn is_amplifier_node(&self, id: Uuid) -> bool {
        self.amplifier_nodes.contains(&id)
    }
    /// Marks or unmarks the node with the given [`Uuid`] as an amplifier candidate.
    ///
    /// Unmarking a node strips its gain model entry from every [`PumpScenario`].
    pub fn set_is_amplifier_node(&mut self, id: Uuid, is_amplifier: bool) {
        if is_amplifier {
            self.amplifier_nodes.insert(id);
        } else {
            self.amplifier_nodes.remove(&id);
            for scenario in self.pump_scenarios.values_mut() {
                scenario.set_gain_model(id, GainModel::None);
            }
        }
    }
    /// Drops entries referencing deleted nodes from the amplifier candidate set.
    pub fn prune_amplifier_nodes(&mut self) {
        let scenery = &self.scenery;
        self.amplifier_nodes.retain(|id| scenery.exists(*id));
    }
    /// Replaces the entire amplifier candidate set at once (e.g., for undo/redo) without modifying scenarios.
    pub fn set_amplifier_nodes(&mut self, nodes: HashSet<Uuid>) {
        self.amplifier_nodes = nodes;
    }
    /// Returns the cached scalar metrics collected during the last [`analyze_headless`](Self::analyze_headless) run.
    #[must_use]
    pub const fn metric_store(&self) -> &MetricStore {
        &self.metric_store
    }
    /// Returns the cached vector metrics (e.g., per-ray OPDs) collected during the last [`analyze_headless`](Self::analyze_headless) run.
    #[must_use]
    pub const fn vector_metric_store(&self) -> &VectorMetricStore {
        &self.vector_metric_store
    }
    /// Returns the optimization configuration of this [`OpmDocument`], if set.
    #[must_use]
    pub const fn optimization_config(&self) -> Option<&OptimizationConfig> {
        self.optimization_config.as_ref()
    }
    /// Sets or clears the optimization configuration of this [`OpmDocument`].
    pub fn set_optimization_config(&mut self, config: Option<OptimizationConfig>) {
        self.optimization_config = config;
    }
}
