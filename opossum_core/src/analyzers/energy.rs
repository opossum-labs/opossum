//! Performing a (simple) energy flow analysis
#![warn(missing_docs)]
use std::collections::HashMap;

use super::{Analyzer, AnalyzerRegistration, AnalyzerType};
use crate::{
    analyzers::propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy},
    core_optics::{OpticNode, node_attr::HasNodeAttr},
    error::OpmResult,
    light::LightResult,
    nodes::NodeGroup,
    prelude::EnergyDataBuilder,
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
    /// Maps an energy data builder to the `SourcePort` node with the given UUID
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
        mut incoming_data: LightResult,
        _config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        let in_ports = self.ports().names(&crate::core_optics::PortType::Input);
        let out_ports = self.ports().names(&crate::core_optics::PortType::Output);

        // If the node doesn't have at least one input and output, we can't pass energy through
        // which would be a programming error in the implementation of the node. We use debug asserts here to catch this.
        debug_assert!(
            !in_ports.is_empty(),
            "OpticNode '{}' ({}) has no input ports defined! The defined energy flow analysis requires at least one input port.",
            self.name(),
            self.node_type()
        );
        debug_assert!(
            !out_ports.is_empty(),
            "OpticNode '{}' ({}) has no output ports defined! The defined energy flow analysis requires at least one output port.",
            self.name(),
            self.node_type()
        );

        let in_port = &in_ports[0];
        let out_port = &out_ports[0];

        let Some(data) = incoming_data.remove(in_port) else {
            return Ok(LightResult::default());
        };
        let apodized = if let Some(surf) = self.get_optic_surface(out_port)
            && !surf.aperture().is_none()
        {
            true
        } else if let Some(surf) = self.get_optic_surface(in_port)
            && !surf.aperture().is_none()
        {
            true
        } else {
            false
        };
        if apodized {
            self.set_apodization_warning(true);
            log::warn!(
                "Rays have been apodized at input aperture of '{}' ({}). Results might not be accurate.",
                self.node_attr().name(),
                self.node_type()
            );
        }
        self.set_light_data(data.clone());
        Ok(LightResult::from([(out_port.clone(), data)]))
    }
}

#[cfg(test)]
mod test {
    use super::EnergyAnalyzer;
    use crate::{
        analyzers::{Analyzer, energy::EnergyConfig},
        error::OpmResult,
        joule,
        light::lightdata::energy_data_builder::{EnergyDataBuilder, EnergyLaserLines},
        nanometer,
        nodes::{EnergyMeter, NodeGroup, SourcePort},
    };
    use num::Zero;
    use uom::si::f64::Length;

    fn create_scene() -> OpmResult<(NodeGroup, EnergyConfig)> {
        let mut scenery = NodeGroup::default();
        let energy_data_builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
            vec![(nanometer!(633.0), joule!(1.0))],
            nanometer!(1.0),
        )?);
        let i_src = scenery.add_node(SourcePort::default())?;
        let i_em = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(i_src, "output_1", i_em, "input_1", Length::zero())?;
        let mut config = EnergyConfig::default();
        config.map_source(i_src, energy_data_builder.into());
        Ok((scenery, config))
    }

    #[test]
    fn analyze_empty_scene() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let energy_analyzer = EnergyAnalyzer::default();
        energy_analyzer.analyze(&mut scenery)?;
        Ok(())
    }
    #[test]
    fn analyze_full_scene() -> OpmResult<()> {
        let (mut scenery, config) = create_scene()?;
        let energy_analyzer = EnergyAnalyzer::new(config);
        energy_analyzer.analyze(&mut scenery)?;
        Ok(())
    }
    #[test]
    fn analyze_report_without_analysis() -> OpmResult<()> {
        let (scenery, config) = create_scene()?;
        let energy_analyzer = EnergyAnalyzer::new(config);
        energy_analyzer.report(&scenery)?;
        Ok(())
    }

    #[test]
    fn test_map_and_get_source() -> OpmResult<()> {
        use uuid::Uuid;
        let mut config = super::EnergyConfig::default();
        let uuid = Uuid::new_v4();
        let builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
            vec![(nanometer!(633.0), joule!(1.0))],
            nanometer!(1.0),
        )?);

        assert_eq!(config.map_source(uuid, builder.clone()), false);
        assert_eq!(config.get_source(&uuid), Some(&builder));

        let builder2 = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
            vec![(nanometer!(532.0), joule!(2.0))],
            nanometer!(1.0),
        )?);
        assert_eq!(config.map_source(uuid, builder2.clone()), true);
        assert_eq!(config.get_source(&uuid), Some(&builder2));
        Ok(())
    }

    #[test]
    fn test_remove_source() -> OpmResult<()> {
        use uuid::Uuid;
        let mut config = super::EnergyConfig::default();
        let uuid = Uuid::new_v4();
        let builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
            vec![(nanometer!(633.0), joule!(1.0))],
            nanometer!(1.0),
        )?);
        config.map_source(uuid, builder.clone());
        assert_eq!(config.remove_source(&uuid), Some(builder));
        assert!(config.get_source(&uuid).is_none());
        assert!(config.remove_source(&uuid).is_none());
        Ok(())
    }

    #[test]
    fn test_prune_source_map() -> OpmResult<()> {
        use uuid::Uuid;
        let mut config = super::EnergyConfig::default();
        let uuid2 = Uuid::new_v4();
        let builder = EnergyDataBuilder::LaserLines(EnergyLaserLines::new(
            vec![(nanometer!(633.0), joule!(1.0))],
            nanometer!(1.0),
        )?);

        let mut scene = NodeGroup::default();
        let node_id = scene.add_node(SourcePort::new("source"))?;

        config.map_source(node_id, builder.clone());
        config.map_source(uuid2, builder.clone());

        config.prune_source_map(&scene);

        assert!(config.get_source(&node_id).is_some());
        assert!(config.get_source(&uuid2).is_none());
        Ok(())
    }
}
