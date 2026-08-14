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
use uuid::Uuid;

/// A named operating point: the gain models of all amplifying nodes in one analysis run.
///
/// Only the amplifying nodes are held. Setting a node to [`GainModel::None`] removes it from the
/// scenario rather than storing an inactive entry, so "is in the map" and "amplifies" cannot
/// disagree.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PumpScenario {
    name: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
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
