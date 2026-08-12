use super::CylindricLens;
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::{OpticNodeExt, node_attr::HasNodeAttr},
    error::OpmResult,
    light::LightResult,
};

impl AnalysisRayTrace for CylindricLens {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let material = self.get_ray_trace_material(self.node_attr())?;
        self.unified_analyze_volume_node(incoming_data, &material, config)
    }
}
