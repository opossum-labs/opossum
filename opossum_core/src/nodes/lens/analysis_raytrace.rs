use super::Lens;
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::Volumetric,
    error::OpmResult,
    light::LightResult,
};

impl AnalysisRayTrace for Lens {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        self.unified_analyze_volume_node(incoming_data, config)
    }
}
