//! Analyzer performing a ghost focus analysis using ray tracing

use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    error::OpmResult,
    light_result::{LightBouncingRays, LightResult},
    nodes::NodeGroup,
    optic_node::OpticNode,
};

use super::{raytrace::AnalysisRayTrace, Analyzer, RayTraceConfig};
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
/// Configuration for performing a ghost focus analysis
pub struct GhostFocusConfig {
    max_bounces: usize,
}

impl GhostFocusConfig {
    /// Returns the max bounces of this [`GhostFocusConfig`].
    #[must_use]
    pub const fn max_bounces(&self) -> usize {
        self.max_bounces
    }
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
    fn analyze(&self, scenery: &mut NodeGroup) -> OpmResult<()> {
        let scenery_name = if scenery.node_attr().name().is_empty() {
            String::new()
        } else {
            format!(" '{}'", scenery.node_attr().name())
        };
        info!("Calculate node positions of scenery{scenery_name}.");
        AnalysisRayTrace::calc_node_position(
            scenery,
            LightResult::default(),
            &RayTraceConfig::default(),
        )?;
        info!("Performing ghost focus analysis of scenery{scenery_name}.");
        AnalysisGhostFocus::analyze(scenery, LightBouncingRays::default(), &self.config)?;
        Ok(())
    }
}

/// Trait for implementing the energy flow analysis.
pub trait AnalysisGhostFocus: OpticNode + AnalysisRayTrace {
    /// Perform a ghost focus analysis of an [`OpticNode`].
    ///
    /// This function is similar to the corresponding [`AnalysisRayTrace`] function but also
    /// returns possible reflected [`Rays`](crate::rays::Rays) as the second component of the return tuple.  
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn analyze(
        &mut self,
        _incoming_data: LightBouncingRays,
        _config: &GhostFocusConfig,
    ) -> OpmResult<LightBouncingRays> {
        warn!(
            "{}: No ghost focus analysis function defined.",
            self.node_type()
        );
        Ok(LightBouncingRays::default())
    }
}
