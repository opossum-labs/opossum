#![warn(missing_docs)]

use super::NodeGroup;
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus},
    error::OpmResult,
    light::{LightRays, Rays},
};

impl AnalysisGhostFocus for NodeGroup {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        ray_collection: &mut Vec<Rays>,
        bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        self.graph
            .analyze_ghostfocus(incoming_data, config, ray_collection, bounce_lvl)
    }
}
