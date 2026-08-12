use super::CylindricLens;
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace}, core_optics::{OpticNodeExt, node_attr::HasNodeAttr}, error::{OpmResult, OpossumError}, light::{LightRays, Rays}, properties::{Proptype, proptype::AssetRef},
};

impl AnalysisGhostFocus for CylindricLens {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let material = self.get_ray_trace_material(self.node_attr())?;
        self.unified_analyze_volume_node_ghost_focus(incoming_data, &material, config)
    }
}
