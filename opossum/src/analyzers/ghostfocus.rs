//! Analyzer performing a ghost focus analysis using ray tracing

use serde::{Deserialize, Serialize};

use crate::{error::OpmResult, nodes::NodeGroup};

use super::Analyzer;
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
/// Configuration for performing a ghost focus analysis
pub struct GhostFocusConfig {
    max_bounces: usize,
}
impl Default for GhostFocusConfig {
    fn default() -> Self {
        Self { max_bounces: 1 }
    }
}
/// Analyzer for ghost focus simulation
#[derive(Default, Debug)]
pub struct GhostFocusAnalyzer {
    config: GhostFocusConfig,
}
impl GhostFocusAnalyzer {
    /// Creates a new [`GhostFocusAnalyzer`].
    #[must_use]
    pub const fn new(config: GhostFocusConfig) -> Self {
        Self { config }
    }
    /// Returns a reference to the config of this [`GhostFocusAnalyzer`].
    #[must_use]
    pub const fn config(&self) -> &GhostFocusConfig {
        &self.config
    }
}
impl Analyzer for GhostFocusAnalyzer {
    fn analyze(&self, _scenery: &mut NodeGroup) -> OpmResult<()> {
        // let scenery_name = if scenery.description().is_empty() {
        //     String::new()
        // } else {
        //     format!(" '{}'", scenery.description())
        // };
        // info!("Performing ghost focus analysis of scenery{scenery_name}.");
        // let graph = scenery.graph_mut();
        // let name = format!("scenery{scenery_name}");
        // graph.calc_node_positions(&name, &LightResult::default())?;
        // let name = format!("Scenery{scenery_name}");
        // graph.analyze(
        //     &name,
        //     &LightResult::default(),
        //     &AnalyzerType::GhostFocus(self.config.clone()),
        // )?;
        Ok(())
    }
}
