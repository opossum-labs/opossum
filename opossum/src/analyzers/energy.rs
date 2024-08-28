//! Performing a (simple) energy flow analysis
use log::info;

use crate::{analyzers::AnalyzerType, error::OpmResult, optical::LightResult, OpticScenery};

use super::Analyzer;

#[derive(Debug, Default)]
pub struct EnergyAnalyzer {}

impl Analyzer for EnergyAnalyzer {
    fn analyze(&self, scenery: &mut OpticScenery) -> OpmResult<()> {
        let scenery_name = if scenery.description().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.description())
        };
        info!("Performing energy analysis of scenery{scenery_name}.");
        let graph = scenery.graph_mut();
        let name = format!("Scenery{scenery_name}");
        graph.analyze(&name, &LightResult::default(), &AnalyzerType::Energy)?;
        Ok(())
    }
}
