#![warn(missing_docs)]
//! Named operating points of a model: which of its components amplify, and how strongly.
//!
//! Hardware and operating point are deliberately kept apart. What a component *is* — its geometry
//! and its material — belongs to the node and travels with the model. What it *does in one
//! particular run* — pumped or passive — belongs to a [`PumpScenario`]. A model with multiple amplifier
//! heads can therefore be run in different pump variants for each amplifier without editing the model itself, and the
//! variants can be compared side by side.
//!
//! This is the split the [`SourcePort`](crate::nodes::SourcePort) node already uses for light
//! sources: the node marks the *place*, the analyzer configuration holds what is emitted there. A
//! scenario does the same for gain, one level up — it is owned by the document rather than by a
//! single analyzer, because the same operating point is worth analyzing in several ways.
//!
//! **What marks a node as an amplifier is its membership in a scenario**, not its material: a node
//! that no scenario mentions is passive, which is why a document without scenarios computes exactly
//! what it always did.

use crate::{gain::GainModel, nodes::NodeGroup};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

/// A named operating point: the gain models of all amplifying nodes in one analysis run.
///
/// Only the amplifying nodes are held. Setting a node to [`GainModel::None`] removes it from the
/// scenario rather than storing an inactive entry, so "is in the map" and "amplifies" cannot
/// disagree.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, ToSchema)]
pub struct PumpScenario {
    name: String,
    /// Keyed by node uuid; opaque in the API schema like every other uuid-keyed map in this crate
    /// (e.g. `EnergyConfig::source_map`) - `utoipa` schemas key JSON objects by string, so a real
    /// schema here would document a shape the wire format doesn't have.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[schema(value_type = Object)]
    gain_models: HashMap<Uuid, GainModel>,
}

impl PumpScenario {
    /// Create a new, empty [`PumpScenario`] with the given name.
    ///
    /// Every node is passive in a fresh scenario, so analyzing it reproduces the passive model.
    ///
    /// # Arguments
    ///
    /// * `name` - the name this scenario is shown and reported under.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            gain_models: HashMap::new(),
        }
    }
    /// Return the name of this [`PumpScenario`].
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Set the name of this [`PumpScenario`].
    ///
    /// # Arguments
    ///
    /// * `name` - the new name.
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
    /// Return the [`GainModel`] the node with the given [`Uuid`] runs with in this scenario.
    ///
    /// A node this scenario does not mention is passive, so this answers [`GainModel::None`] rather
    /// than nothing — a caller asking "how does this node amplify here" always gets an answer it can
    /// use directly.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to look up.
    ///
    /// # Returns
    ///
    /// The node's gain model in this scenario.
    #[must_use]
    pub fn gain_model(&self, node_id: Uuid) -> GainModel {
        self.gain_models
            .get(&node_id)
            .copied()
            .unwrap_or(GainModel::None)
    }
    /// Set the [`GainModel`] of the node with the given [`Uuid`] in this scenario.
    ///
    /// Setting [`GainModel::None`] takes the node out of the scenario again, which is the same
    /// thing: a node that does not amplify in this operating point.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to configure.
    /// * `model` - how it amplifies, or [`GainModel::None`] to make it passive.
    pub fn set_gain_model(&mut self, node_id: Uuid, model: GainModel) {
        if model.is_active() {
            self.gain_models.insert(node_id, model);
        } else {
            self.gain_models.remove(&node_id);
        }
    }
    /// Return all nodes that amplify in this scenario, together with their [`GainModel`].
    ///
    /// # Returns
    ///
    /// An iterator over the amplifying nodes in unspecified order.
    pub fn amplifiers(&self) -> impl Iterator<Item = (Uuid, GainModel)> + '_ {
        self.gain_models.iter().map(|(id, model)| (*id, *model))
    }
    /// Remove all entries whose nodes no longer exist in the given model.
    ///
    /// A scenario refers to nodes by [`Uuid`], so deleting a node would leave an entry behind that
    /// no longer belongs to anything. Same purpose as
    /// [`prune_source_map`](crate::analyzers::energy::EnergyConfig::prune_source_map) for the light
    /// sources.
    ///
    /// # Arguments
    ///
    /// * `model` - the model the remaining entries have to exist in.
    pub fn prune(&mut self, model: &NodeGroup) {
        self.gain_models.retain(|node_id, _| model.exists(*node_id));
    }
}

/// The [`PumpScenario`] an analysis is *currently* being run in.
///
/// This is the slot an analyzer configuration carries so that the operating point reaches the
/// components during a run: the configuration is the object handed all the way down into the
/// propagation, so it is the only thing that can carry it there. Filled on a copy of the
/// configuration by [`OpmDocument::analyze`](crate::opm_document::OpmDocument::analyze) and never
/// written to a file.
///
/// It is a type of its own rather than a plain `Option<PumpScenario>` for one reason: an analyzer
/// configuration is *what the user set up*, and two configurations describing the same set-up have
/// to compare equal even if one of them happens to be in the middle of a run. Hence the
/// [`PartialEq`] implementation below, which deliberately ignores the content. Without it a
/// comparison of two configurations - the backend does exactly that to decide what an undo has to
/// restore - could differ over run state that no user ever entered.
#[derive(Debug, Clone, Default)]
pub struct ActiveScenario(Option<PumpScenario>);

impl PartialEq for ActiveScenario {
    /// Two [`ActiveScenario`]s always compare equal: the operating point of a run is not part of
    /// the identity of the configuration holding it.
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl ActiveScenario {
    /// Set the operating point of the run about to be performed.
    ///
    /// # Arguments
    ///
    /// * `scenario` - the operating point, or `None` for a passive run.
    pub fn set(&mut self, scenario: Option<PumpScenario>) {
        self.0 = scenario;
    }
    /// Return the [`GainModel`] the node with the given [`Uuid`] runs with.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to look up.
    ///
    /// # Returns
    ///
    /// The node's gain model, or [`GainModel::None`] if no operating point is set at all.
    #[must_use]
    pub fn gain_model(&self, node_id: Uuid) -> GainModel {
        self.0
            .as_ref()
            .map_or(GainModel::None, |scenario| scenario.gain_model(node_id))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        error::OpmResult,
        gain::ConstGain,
        nodes::{Dummy, Lens},
    };

    #[test]
    fn a_fresh_scenario_is_entirely_passive() {
        let scenario = PumpScenario::new("cold");
        assert_eq!(scenario.name(), "cold");
        assert_eq!(scenario.gain_model(Uuid::new_v4()), GainModel::None);
        assert_eq!(scenario.amplifiers().count(), 0);
    }
    #[test]
    fn setting_a_model_makes_a_node_an_amplifier() -> OpmResult<()> {
        let mut scenario = PumpScenario::new("full power");
        let node_id = Uuid::new_v4();
        let model = GainModel::Const(ConstGain::new(2.5)?);
        scenario.set_gain_model(node_id, model);
        assert_eq!(scenario.gain_model(node_id), model);
        assert_eq!(
            scenario.amplifiers().collect::<Vec<_>>(),
            vec![(node_id, model)]
        );
        Ok(())
    }
    #[test]
    fn setting_no_gain_takes_a_node_out_of_the_scenario() -> OpmResult<()> {
        // "passive in this operating point" and "not part of this operating point" must not be two
        // different states, otherwise a listing of the amplifiers shows nodes that do not amplify.
        let mut scenario = PumpScenario::new("full power");
        let node_id = Uuid::new_v4();
        scenario.set_gain_model(node_id, GainModel::Const(ConstGain::new(2.5)?));
        scenario.set_gain_model(node_id, GainModel::None);
        assert_eq!(scenario.gain_model(node_id), GainModel::None);
        assert_eq!(scenario.amplifiers().count(), 0);
        Ok(())
    }
    #[test]
    fn prune_drops_entries_of_deleted_nodes() -> OpmResult<()> {
        let mut model = NodeGroup::default();
        let lens_id = model.add_node(Lens::default())?;
        let deleted_id = model.add_node(Dummy::default())?;
        model.delete_node(deleted_id)?;

        let mut scenario = PumpScenario::new("full power");
        let gain = GainModel::Const(ConstGain::new(2.5)?);
        scenario.set_gain_model(lens_id, gain);
        scenario.set_gain_model(deleted_id, gain);
        scenario.prune(&model);

        assert_eq!(scenario.gain_model(lens_id), gain);
        assert_eq!(scenario.gain_model(deleted_id), GainModel::None);
        Ok(())
    }
    #[test]
    fn an_unset_active_scenario_amplifies_nothing() {
        assert_eq!(
            ActiveScenario::default().gain_model(Uuid::new_v4()),
            GainModel::None
        );
    }
    #[test]
    fn an_active_scenario_answers_for_its_nodes() -> OpmResult<()> {
        let node_id = Uuid::new_v4();
        let model = GainModel::Const(ConstGain::new(2.5)?);
        let mut scenario = PumpScenario::new("full power");
        scenario.set_gain_model(node_id, model);
        let mut active = ActiveScenario::default();
        active.set(Some(scenario));
        assert_eq!(active.gain_model(node_id), model);
        assert_eq!(active.gain_model(Uuid::new_v4()), GainModel::None);
        Ok(())
    }
    /// What a run is doing right now is not part of what a user configured, so it must not make two
    /// otherwise identical configurations differ - the backend compares them to drive undo.
    #[test]
    fn the_active_scenario_does_not_take_part_in_comparisons() {
        let mut active = ActiveScenario::default();
        active.set(Some(PumpScenario::new("full power")));
        assert_eq!(active, ActiveScenario::default());
    }
    #[test]
    fn serde_roundtrip() -> OpmResult<()> {
        let mut scenario = PumpScenario::new("full power");
        scenario.set_gain_model(Uuid::new_v4(), GainModel::Const(ConstGain::new(2.5)?));
        let serialized = ron::to_string(&scenario)
            .map_err(|e| crate::error::OpossumError::Other(e.to_string()))?;
        let deserialized: PumpScenario = ron::from_str(&serialized)
            .map_err(|e| crate::error::OpossumError::Other(e.to_string()))?;
        assert_eq!(scenario, deserialized);
        Ok(())
    }
}
