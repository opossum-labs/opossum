//! Performing a (simple) energy flow analysis
use log::info;

use crate::{error::OpmResult, light_result::LightResult, nodes::NodeGroup, optic_node::OpticNode};

use super::Analyzer;

//pub type LightResEnergy = LightDings<DataEnergy>;

/// Analyzer for simulating a simple energy flow
#[derive(Debug, Default)]
pub struct EnergyAnalyzer {}

impl Analyzer for EnergyAnalyzer {
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()> {
        let scenery_name = if scenery.node_attr().name().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.node_attr().name())
        };
        info!("Performing energy flow analysis of scenery{scenery_name}.");
        AnalysisEnergy::analyze(scenery, LightResult::default())?;
        Ok(())
    }
}
/// Trait for implementing the energy flow analysis.
pub trait AnalysisEnergy: OpticNode {
    /// Analyze the energy flow of an [`OpticNode`].
    ///
    /// # Errors
    /// This function will return an error if the concrete implementation of the [`OpticNode`] fails.
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult>;
}
