use super::Wedge;
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus},
    core_optics::Volumetric,
    error::OpmResult,
    light::{LightRays, Rays},
};

impl AnalysisGhostFocus for Wedge {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        self.unified_analyze_volume_node_ghost_focus(incoming_data, config)
    }
}
