//! Performing a (simple) energy flow analysis
#![warn(missing_docs)]
use std::collections::HashMap;

use super::Analyzer;
use super::{AnalyzerRegistration, AnalyzerType};
use crate::analyzers::propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy};
use crate::prelude::EnergyDataBuilder;
use crate::{
    error::OpmResult, light_result::LightResult, nodes::NodeGroup, optic_node::OpticNode,
    reporting::analysis_report::AnalysisReport,
};
use log::info;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

inventory::submit! {
    AnalyzerRegistration::new(
        || AnalyzerType::Energy(EnergyConfig::default()),
        |at| if let AnalyzerType::Energy(config) = at { Some(Box::new(EnergyAnalyzer::new(config.clone()))) } else { None }
    )
}

/// Configuration for the energy flow analysis.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct EnergyConfig {
    source_map: HashMap<Uuid, EnergyDataBuilder>,
}
impl EnergyConfig {
    /// Maps an energy data builder to the SourcePort node with the given UUID
    ///
    /// If a builder was already mapped this function returns `true`. A new mapping
    /// reutrns `false`
    pub fn map_source(&mut self, node_id: Uuid, energy_data_builder: EnergyDataBuilder) -> bool {
        self.source_map
            .insert(node_id, energy_data_builder)
            .is_some()
    }
    /// Returns the energy data builder mapped to the given source UUID, if any.
    #[must_use]
    pub fn get_source(&self, uuid: &Uuid) -> Option<&EnergyDataBuilder> {
        self.source_map.get(uuid)
    }
    /// Removes and returns the energy data builder mapped to the given source UUID, if any.
    #[must_use]
    pub fn remove_source(&mut self, uuid: &Uuid) -> Option<EnergyDataBuilder> {
        self.source_map.remove(uuid)
    }
    /// Removes all source mappings whose UUIDs no longer exist in the given model.
    pub fn prune_source_map(&mut self, model: &NodeGroup) {
        self.source_map.retain(|uuid, _builder| model.exists(*uuid));
    }
}

impl PropagationStrategy for EnergyConfig {
    fn missed_surface_strategy(&self) -> MissedSurfaceStrategy {
        MissedSurfaceStrategy::Stop
    }
}
/// Analyzer for simulating a simple energy flow
#[derive(Debug, Default)]
pub struct EnergyAnalyzer {
    config: EnergyConfig,
}

impl EnergyAnalyzer {
    /// Create a new energy analyzer with the given configuration.
    #[must_use]
    pub const fn new(config: EnergyConfig) -> Self {
        Self { config }
    }
}

impl Analyzer for EnergyAnalyzer {
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()> {
        let scenery_name = if scenery.node_attr().name().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.node_attr().name())
        };
        info!("Performing energy flow analysis of scenery{scenery_name}.");
        AnalysisEnergy::analyze(scenery, LightResult::default(), &self.config)?;
        Ok(())
    }
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport> {
        let mut report = scenery.toplevel_report()?;
        report.set_analysis_type("Energy Analysis");
        Ok(report)
    }
}
/// Trait for implementing the energy flow analysis.
pub trait AnalysisEnergy: OpticNode {
    /// Analyze the energy flow of an [`OpticNode`].
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the [`OpticNode`] fails.
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        self.unified_analyze_single_surface_node(incoming_data, config, "input_1", None)
    }
}

#[cfg(test)]
mod test {
    use num::Zero;
    use uom::si::f64::Length;

    use super::EnergyAnalyzer;
    use crate::{
        analyzers::Analyzer,
        joule,
        lightdata::{
            energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
            light_data_builder::LightDataBuilder,
        },
        nanometer,
        nodes::{EnergyMeter, NodeGroup, Source},
    };
    #[test]
    fn analyze_empty_scene() {
        let mut scenery = NodeGroup::default();
        let energy_analyzer = EnergyAnalyzer::default();
        energy_analyzer.analyze(&mut scenery).unwrap();
    }
    fn create_scene() -> NodeGroup {
        let mut scenery = NodeGroup::default();
        let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(633.0), joule!(1.0))], nanometer!(1.0)).unwrap(),
        ));
        let src = Source::new("source", light_data_builder);
        let i_src = scenery.add_node(src).unwrap();
        let i_em = scenery.add_node(EnergyMeter::default()).unwrap();
        scenery
            .connect_nodes(i_src, "output_1", i_em, "input_1", Length::zero())
            .unwrap();
        scenery
    }
    #[test]
    fn analyze_full_scene() {
        let mut scenery = create_scene();
        let energy_analyzer = EnergyAnalyzer::default();
        energy_analyzer.analyze(&mut scenery).unwrap();
    }
    #[test]
    fn analyze_report_without_analysis() {
        let mut scenery = create_scene();
        let energy_analyzer = EnergyAnalyzer::default();
        energy_analyzer.analyze(&mut scenery).unwrap();
        energy_analyzer.report(&scenery).unwrap();
    }

    #[test]
    fn test_map_and_get_source() {
        use uuid::Uuid;
        let mut config = super::EnergyConfig::default();
        let uuid = Uuid::new_v4();
        let builder = EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(633.0), joule!(1.0))], nanometer!(1.0)).unwrap(),
        );

        assert_eq!(config.map_source(uuid, builder.clone()), false);
        assert_eq!(config.get_source(&uuid), Some(&builder));

        let builder2 = EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(532.0), joule!(2.0))], nanometer!(1.0)).unwrap(),
        );
        assert_eq!(config.map_source(uuid, builder2.clone()), true);
        assert_eq!(config.get_source(&uuid), Some(&builder2));
    }

    #[test]
    fn test_remove_source() {
        use uuid::Uuid;
        let mut config = super::EnergyConfig::default();
        let uuid = Uuid::new_v4();
        let builder = EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(633.0), joule!(1.0))], nanometer!(1.0)).unwrap(),
        );

        config.map_source(uuid, builder.clone());
        assert_eq!(config.remove_source(&uuid), Some(builder));
        assert!(config.get_source(&uuid).is_none());
        assert!(config.remove_source(&uuid).is_none());
    }

    #[test]
    fn test_prune_source_map() {
        use uuid::Uuid;
        let mut config = super::EnergyConfig::default();
        let uuid2 = Uuid::new_v4();
        let builder = EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(633.0), joule!(1.0))], nanometer!(1.0)).unwrap(),
        );

        let mut scene = NodeGroup::default();
        let src = Source::new("source", LightDataBuilder::Energy(builder.clone()));
        let node_id = scene.add_node(src).unwrap();

        config.map_source(node_id, builder.clone());
        config.map_source(uuid2, builder.clone());

        config.prune_source_map(&scene);

        assert!(config.get_source(&node_id).is_some());
        assert!(config.get_source(&uuid2).is_none());
    }
}
