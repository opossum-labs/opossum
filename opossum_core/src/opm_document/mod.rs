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

#[cfg(test)]
mod tests;

pub use analyzer_info::AnalyzerInfo;

use crate::{
    analyzers::AnalyzerType,
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    gain::{GainModel, PumpScenario},
    material::Material,
    nodes::NodeGroup,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// The main structure of an OPOSSUM model.
/// It contains the [`NodeGroup`] representing the optical model, a list of analyzers, and a global configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpmDocument {
    pub(crate) opm_file_version: String,
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
}

impl Default for OpmDocument {
    fn default() -> Self {
        Self {
            opm_file_version: env!("OPM_FILE_VERSION").to_string(),
            scenery: NodeGroup::default(),
            analyzers: IndexMap::default(),
            pump_scenarios: IndexMap::default(),
            amplifier_nodes: HashSet::default(),
            embedded_materials: IndexMap::default(),
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

    /// Returns a reference to the scenery of this [`OpmDocument`].
    #[must_use]
    pub const fn scenery(&self) -> &NodeGroup {
        &self.scenery
    }

    /// Returns a mutable reference to the scenery of this [`OpmDocument`].
    pub const fn scenery_mut(&mut self) -> &mut NodeGroup {
        &mut self.scenery
    }

    /// Returns the list of analyzers of this [`OpmDocument`].
    #[must_use]
    pub fn analyzers(&self) -> IndexMap<Uuid, AnalyzerInfo> {
        self.analyzers.clone()
    }

    /// Returns a mutable reference to the analyzers of this [`OpmDocument`].
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

    /// Returns an [`AnalyzerInfo`] with the given [`Uuid`] from this [`OpmDocument`].
    ///
    /// # Errors
    ///
    /// Returns an error if the [`AnalyzerInfo`] with the given [`Uuid`] was not found.
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

    /// Adds an analyzer to this [`OpmDocument`].
    pub fn add_analyzer(&mut self, analyzer_type: AnalyzerType) -> Uuid {
        let id = Uuid::new_v4();
        let analyzer_info = AnalyzerInfo::new_without_position(analyzer_type);
        self.analyzers.insert(id, analyzer_info);
        id
    }

    /// Adds an analyzer (with an optional GUI position) to this [`OpmDocument`].
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

    /// Removes an analyzer from this [`OpmDocument`].
    ///
    /// # Errors
    ///
    /// Returns an error if an [`AnalyzerType`] with the given [`Uuid`] was not found.
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
    /// it is used to restore an analyzer to the exact identity it had before (e.g. for undo/redo).
    pub fn insert_analyzer(&mut self, id: Uuid, info: AnalyzerInfo) {
        self.analyzers.insert(id, info);
    }

    /// Returns all [`PumpScenario`]s of this [`OpmDocument`].
    #[must_use]
    pub const fn pump_scenarios(&self) -> &IndexMap<Uuid, PumpScenario> {
        &self.pump_scenarios
    }

    /// Returns the [`PumpScenario`] with the given [`Uuid`], if there is one.
    #[must_use]
    pub fn pump_scenario(&self, id: Uuid) -> Option<&PumpScenario> {
        self.pump_scenarios.get(&id)
    }

    /// Returns a mutable reference to the [`PumpScenario`] with the given [`Uuid`], if there is one.
    pub fn pump_scenario_mut(&mut self, id: Uuid) -> Option<&mut PumpScenario> {
        self.pump_scenarios.get_mut(&id)
    }

    /// Adds a new, empty [`PumpScenario`] with the given name to this [`OpmDocument`].
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
    /// Every analyzer running in that scenario stops doing so.
    pub fn remove_pump_scenario(&mut self, id: Uuid) -> Option<PumpScenario> {
        let removed = self.pump_scenarios.shift_remove(&id)?;
        for analyzer in self.analyzers.values_mut() {
            analyzer.remove_pump_scenario(id);
        }
        Some(removed)
    }

    /// Drops the entries of deleted nodes from every [`PumpScenario`] of this [`OpmDocument`].
    pub fn prune_pump_scenarios(&mut self) {
        for scenario in self.pump_scenarios.values_mut() {
            scenario.prune(&self.scenery);
        }
    }

    /// Returns the amplifier candidate set of this [`OpmDocument`].
    #[must_use]
    pub const fn amplifier_nodes(&self) -> &HashSet<Uuid> {
        &self.amplifier_nodes
    }

    /// Returns whether the node with the given [`Uuid`] is an amplifier candidate.
    #[must_use]
    pub fn is_amplifier_node(&self, id: Uuid) -> bool {
        self.amplifier_nodes.contains(&id)
    }

    /// Marks or unmarks the node with the given [`Uuid`] as an amplifier candidate.
    ///
    /// Unmarking a node strips it from every [`PumpScenario`]'s gain-model map.
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

    /// Drops the entries of deleted nodes from the amplifier candidate set.
    pub fn prune_amplifier_nodes(&mut self) {
        let scenery = &self.scenery;
        self.amplifier_nodes.retain(|id| scenery.exists(*id));
    }

    /// Replaces the whole amplifier candidate set at once (e.g. for undo/redo).
    pub fn set_amplifier_nodes(&mut self, nodes: HashSet<Uuid>) {
        self.amplifier_nodes = nodes;
    }

    /// Checks if any node or analyzer in the document is missing its GUI coordinates.
    ///
    /// This signals to frontends that an automatic layout is required.
    #[must_use]
    pub fn needs_autolayout(&self) -> bool {
        // 1. Check if any analyzer is missing its GUI position
        if self.analyzers.values().any(|a| a.gui_position().is_none()) {
            return true;
        }
        // 2. Check optical nodes
        for node_ref in self.scenery.nodes() {
            if node_ref.gui_position().is_none() {
                return true;
            }
        }
        false
    }
}
