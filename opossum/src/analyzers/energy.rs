//! Performing a (simple) energy flow analysis
use log::info;

use crate::{
    analyzers::AnalyzerType,
    error::OpmResult,
    nodes::NodeGroup,
    optical::{LightResult, Optical},
};

use super::Analyzer;

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
        info!("Performing energy analysis of scenery{scenery_name}.");
        let graph = scenery.graph_mut();
        let name = format!("Scenery{scenery_name}");
        graph.analyze(&name, &LightResult::default(), &AnalyzerType::Energy)?;
        Ok(())
    }
}
