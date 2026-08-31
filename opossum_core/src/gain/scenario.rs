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

use crate::{
    gain::{GainModel, PumpSource},
    nodes::NodeGroup,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;

/// What one node does in one operating point: how it is pumped, and how it amplifies.
///
/// The two halves are deliberately independent. A [`PumpSource`] fills the medium with inversion, a
/// [`GainModel`] decides what a beam passing through makes of it, and which pair is chosen is the
/// whole point of an operating point — the same pumping can be evaluated with a cruder or a finer
/// extraction model, and the same model can be run at several pump levels, without editing either.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, ToSchema)]
pub struct PumpConfig {
    #[serde(default)]
    gain_model: GainModel,
    #[serde(default)]
    pump: PumpSource,
}

impl PumpConfig {
    /// Create a new [`PumpConfig`].
    ///
    /// # Arguments
    ///
    /// * `gain_model` - how the node amplifies.
    /// * `pump` - how its medium is pumped.
    #[must_use]
    pub const fn new(gain_model: GainModel, pump: PumpSource) -> Self {
        Self { gain_model, pump }
    }
    /// Return how the node amplifies.
    #[must_use]
    pub const fn gain_model(&self) -> GainModel {
        self.gain_model
    }
    /// Return how the node's medium is pumped.
    #[must_use]
    pub const fn pump(&self) -> PumpSource {
        self.pump
    }
    /// Set how the node amplifies.
    ///
    /// # Arguments
    ///
    /// * `gain_model` - the new gain model.
    pub const fn set_gain_model(&mut self, gain_model: GainModel) {
        self.gain_model = gain_model;
    }
    /// Set how the node's medium is pumped.
    ///
    /// # Arguments
    ///
    /// * `pump` - the new pump source.
    pub const fn set_pump(&mut self, pump: PumpSource) {
        self.pump = pump;
    }
    /// Return whether this configuration does anything at all.
    ///
    /// Either half is enough: a node that is pumped but has no extraction model yet still differs
    /// from one nobody configured, and it must not silently vanish from the operating point while
    /// its pumping is being set up.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.gain_model.is_active() || self.pump.is_active()
    }
}

/// A named operating point: what every configured node does in one analysis run.
///
/// Only the nodes that do something are held. A configuration that neither pumps nor amplifies is
/// removed from the scenario rather than stored as an inactive entry, so "is in the map" and "takes
/// part in this operating point" cannot disagree.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, ToSchema)]
pub struct PumpScenario {
    name: String,
    /// Keyed by node uuid; opaque in the API schema like every other uuid-keyed map in this crate
    /// (e.g. `EnergyConfig::source_map`) - `utoipa` schemas key JSON objects by string, so a real
    /// schema here would document a shape the wire format doesn't have.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    #[schema(value_type = Object)]
    configs: HashMap<Uuid, PumpConfig>,
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
            configs: HashMap::new(),
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
        self.config(node_id).gain_model()
    }
    /// Return how the medium of the node with the given [`Uuid`] is pumped in this scenario.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to look up.
    ///
    /// # Returns
    ///
    /// The node's pump source, or [`PumpSource::None`] if this scenario does not pump it.
    #[must_use]
    pub fn pump_source(&self, node_id: Uuid) -> PumpSource {
        self.config(node_id).pump()
    }
    /// Return the whole [`PumpConfig`] of the node with the given [`Uuid`] in this scenario.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to look up.
    ///
    /// # Returns
    ///
    /// The node's configuration. A node this scenario does not mention answers the default one,
    /// which neither pumps nor amplifies — so a caller always gets something it can use directly.
    #[must_use]
    pub fn config(&self, node_id: Uuid) -> PumpConfig {
        self.configs.get(&node_id).copied().unwrap_or_default()
    }
    /// Set the [`GainModel`] of the node with the given [`Uuid`] in this scenario.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to configure.
    /// * `model` - how it amplifies, or [`GainModel::None`] to make it passive.
    pub fn set_gain_model(&mut self, node_id: Uuid, model: GainModel) {
        self.update(node_id, |config| config.set_gain_model(model));
    }
    /// Set the [`PumpSource`] of the node with the given [`Uuid`] in this scenario.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to configure.
    /// * `pump` - how its medium is pumped, or [`PumpSource::None`] to leave it unpumped.
    pub fn set_pump_source(&mut self, node_id: Uuid, pump: PumpSource) {
        self.update(node_id, |config| config.set_pump(pump));
    }
    /// Set the whole [`PumpConfig`] of the node with the given [`Uuid`] in this scenario.
    ///
    /// For carrying a node's configuration over as a whole — a copy of it appearing under a fresh
    /// uuid, say. A configuration that does nothing takes the node out of the scenario, exactly as
    /// setting either half to inactive would.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to configure.
    /// * `config` - what it does in this operating point.
    pub fn set_config(&mut self, node_id: Uuid, config: PumpConfig) {
        self.update(node_id, |current| *current = config);
    }
    /// Change one node's [`PumpConfig`] and drop it again if nothing is left of it.
    ///
    /// The single place the "inactive means absent" rule lives, so setting a gain model and setting
    /// a pump source cannot come to different conclusions about when an entry stops belonging to
    /// the scenario. Note that it takes *both* halves to be inactive: switching a pumped node's
    /// extraction model off must not throw its pumping away with it.
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to configure.
    /// * `change` - what to change about its configuration.
    fn update(&mut self, node_id: Uuid, change: impl FnOnce(&mut PumpConfig)) {
        let mut config = self.config(node_id);
        change(&mut config);
        if config.is_active() {
            self.configs.insert(node_id, config);
        } else {
            self.configs.remove(&node_id);
        }
    }
    /// Return all nodes taking part in this scenario, together with their [`PumpConfig`].
    ///
    /// # Returns
    ///
    /// An iterator over the configured nodes in unspecified order.
    pub fn amplifiers(&self) -> impl Iterator<Item = (Uuid, PumpConfig)> + '_ {
        self.configs.iter().map(|(id, config)| (*id, *config))
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
        self.configs.retain(|node_id, _| model.exists(*node_id));
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
    /// Return the whole [`PumpConfig`] the node with the given [`Uuid`] runs under.
    ///
    /// The operating point of a node is handed out as one object because a gain model reading the
    /// state of the medium needs both halves at once - see
    /// [`PropagationStrategy::pump_config`](crate::analyzers::propagation_strategy::PropagationStrategy::pump_config).
    ///
    /// # Arguments
    ///
    /// * `node_id` - the node to look up.
    ///
    /// # Returns
    ///
    /// The node's configuration, or the default one - neither pumping nor amplifying - if no
    /// operating point is set at all.
    #[must_use]
    pub fn config(&self, node_id: Uuid) -> PumpConfig {
        self.0
            .as_ref()
            .map_or_else(PumpConfig::default, |scenario| scenario.config(node_id))
    }
    /// Return the [`GainModel`] the node with the given [`Uuid`] runs with.
    ///
    /// Derived from [`ActiveScenario::config`] rather than looked up separately, so the two cannot
    /// disagree.
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
        self.config(node_id).gain_model()
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
            vec![(node_id, PumpConfig::new(model, PumpSource::None))]
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
    fn pumping_and_amplifying_are_configured_independently() -> OpmResult<()> {
        let mut scenario = PumpScenario::new("full power");
        let node_id = Uuid::new_v4();
        let pump = PumpSource::Const;
        let model = GainModel::Const(ConstGain::new(2.5)?);

        // Setting one half must leave the other alone, in either order ...
        scenario.set_pump_source(node_id, pump);
        assert_eq!(scenario.pump_source(node_id), pump);
        assert_eq!(scenario.gain_model(node_id), GainModel::None);
        scenario.set_gain_model(node_id, model);
        assert_eq!(scenario.pump_source(node_id), pump);
        assert_eq!(scenario.gain_model(node_id), model);
        // ... and the node is listed once, with both.
        assert_eq!(
            scenario.amplifiers().collect::<Vec<_>>(),
            vec![(node_id, PumpConfig::new(model, pump))]
        );
        Ok(())
    }
    #[test]
    fn a_node_stays_in_the_scenario_while_either_half_does_something() -> OpmResult<()> {
        // Switching the extraction model off while the medium is still pumped must not throw the
        // pumping away with it - that would silently undo work the user did on the other half.
        let mut scenario = PumpScenario::new("full power");
        let node_id = Uuid::new_v4();
        let pump = PumpSource::Const;
        scenario.set_pump_source(node_id, pump);
        scenario.set_gain_model(node_id, GainModel::Const(ConstGain::new(2.5)?));

        scenario.set_gain_model(node_id, GainModel::None);
        assert_eq!(scenario.pump_source(node_id), pump);
        assert_eq!(scenario.amplifiers().count(), 1);

        // Only once nothing is left of it does the node leave the operating point.
        scenario.set_pump_source(node_id, PumpSource::None);
        assert_eq!(scenario.config(node_id), PumpConfig::default());
        assert_eq!(scenario.amplifiers().count(), 0);
        Ok(())
    }
    #[test]
    fn an_unconfigured_node_is_neither_pumped_nor_amplifying() {
        let scenario = PumpScenario::new("cold");
        let config = scenario.config(Uuid::new_v4());
        assert_eq!(config.gain_model(), GainModel::None);
        assert_eq!(config.pump(), PumpSource::None);
        assert!(!config.is_active());
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
        let node_id = Uuid::new_v4();
        // Neither half of the operating point does anything without a scenario ...
        assert_eq!(
            ActiveScenario::default().config(node_id),
            PumpConfig::default()
        );
        // ... and the derived answer says the same.
        assert_eq!(
            ActiveScenario::default().gain_model(node_id),
            GainModel::None
        );
    }
    #[test]
    fn an_active_scenario_answers_for_its_nodes() -> OpmResult<()> {
        let node_id = Uuid::new_v4();
        let model = GainModel::Const(ConstGain::new(2.5)?);
        let pump = PumpSource::Const;
        let mut scenario = PumpScenario::new("full power");
        scenario.set_gain_model(node_id, model);
        scenario.set_pump_source(node_id, pump);
        let mut active = ActiveScenario::default();
        active.set(Some(scenario));
        // Both halves arrive together, out of the same scenario ...
        assert_eq!(active.config(node_id), PumpConfig::new(model, pump));
        // ... and the gain model derived from that config is the very same one.
        assert_eq!(active.gain_model(node_id), model);
        // A node the scenario does not mention is passive in both halves.
        let stranger = Uuid::new_v4();
        assert_eq!(active.config(stranger), PumpConfig::default());
        assert_eq!(active.gain_model(stranger), GainModel::None);
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
