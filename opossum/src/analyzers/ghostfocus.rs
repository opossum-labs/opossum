//! Analyzer performing a ghost focus analysis using ray tracing
use chrono::Local;
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    error::OpmResult,
    get_version,
    light_result::{LightRays, LightResult},
    nodes::NodeGroup,
    optic_node::OpticNode,
    properties::{Properties, Proptype},
    rays::Rays,
    reporting::analysis_report::{AnalysisReport, NodeReport},
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
    /// Sets the maximum number of ray bounces to be considered during ghost focus analysis.
    pub fn set_max_bounces(&mut self, max_bounces: usize) {
        self.max_bounces = max_bounces;
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
        info!(
            "Performing ghost focus analysis of scenery{scenery_name} up to {} ray bounces.",
            self.config.max_bounces
        );
        AnalysisGhostFocus::analyze(scenery, LightRays::default(), &self.config)?;
        Ok(())
    }
    fn report(&self, scenery: &NodeGroup) -> OpmResult<AnalysisReport> {
        let mut analysis_report = AnalysisReport::new(get_version(), Local::now());
        analysis_report.add_scenery(scenery);
        info!("Add global ray propagation");
        let mut props = Properties::default();
        let all_rays = scenery.accumulated_rays();
        if let Ok(proptype) = <Rays as TryInto<Proptype>>::try_into(all_rays.clone()) {
            props.create("propagation", "ray propagation", None, proptype)?;
        }
        let node_report =
            NodeReport::new("ray propagation", "Global ray propagation", "global", props);
        analysis_report.add_node_report(node_report);
        info!("Add hitmaps...");
        for node in scenery.graph().nodes() {
            let node_name = &node.optical_ref.borrow().name();
            info!("node {node_name}");
            let uuid = node.uuid().as_simple().to_string();
            let mut props = Properties::default();
            let hit_maps = node.optical_ref.borrow().hit_maps();
            for hit_map in &hit_maps {
                props.create(hit_map.0, "surface hit map", None, hit_map.1.clone().into())?;
            }
            let node_report = NodeReport::new("hitmap", &node_name, &uuid, props);
            analysis_report.add_node_report(node_report);
        }
        Ok(analysis_report)
    }
}

/// Trait for implementing the energy flow analysis.
pub trait AnalysisGhostFocus: OpticNode + AnalysisRayTrace {
    /// Perform a ghost focus analysis of an [`OpticNode`].
    ///
    /// This function is similar to the corresponding [`AnalysisRayTrace`] function but also
    /// considers possible reflected [`Rays`](crate::rays::Rays).
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    fn analyze(
        &mut self,
        _incoming_data: LightRays,
        _config: &GhostFocusConfig,
    ) -> OpmResult<LightRays> {
        warn!(
            "{}: No ghost focus analysis function defined.",
            self.node_type()
        );
        Ok(LightRays::default())
    }
}
