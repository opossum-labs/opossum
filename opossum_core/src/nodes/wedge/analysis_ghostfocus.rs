use super::Wedge;
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace},
    core_optics::OpticNodeExt,
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
        let material = self.get_ray_trace_material()?;
        self.unified_analyze_volume_node_ghost_focus(incoming_data, &material, config)
    }
}
