use super::Lens;
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::{OpticNodeExt, node_attr::HasNodeAttr},
    error::OpmResult,
    light::LightResult,
};

impl AnalysisRayTrace for Lens {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (material, _, _) = self.get_node_attributes_ray_trace(self.node_attr())?;
        self.unified_analyze_volume_node(incoming_data, &material, config)
    }
}
