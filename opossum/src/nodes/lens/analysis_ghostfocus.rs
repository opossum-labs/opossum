use super::Lens;
use crate::{
    analyzers::{ghostfocus::AnalysisGhostFocus, AnalyzerType, GhostFocusConfig, RayTraceConfig},
    error::{OpmResult, OpossumError},
    light_result::LightRays,
    optic_node::OpticNode,
    properties::Proptype,
    rays::Rays,
};

impl AnalysisGhostFocus for Lens {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        _config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
    ) -> OpmResult<LightRays> {
        let (in_port, out_port) = if self.inverted() {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let Some(incoming_rays) = incoming_data.get(in_port) else {
            return Ok(LightRays::default());
        };
        let rays = incoming_rays;
        let Some(eff_iso) = self.effective_iso() else {
            return Err(OpossumError::Analysis(
                "no location for surface defined".into(),
            ));
        };
        let Ok(Proptype::RefractiveIndex(index_model)) =
            self.node_attr.get_property("refractive index")
        else {
            return Err(OpossumError::Analysis(
                "cannot read refractive index".into(),
            ));
        };
        let Ok(Proptype::Length(center_thickness)) =
            self.node_attr.get_property("center thickness")
        else {
            return Err(OpossumError::Analysis(
                "cannot read center thickness".into(),
            ));
        };
        let output = if self.inverted() {
            self.analyze_inverse(
                rays.clone(),
                *center_thickness,
                &index_model.value.clone(),
                &eff_iso,
                &AnalyzerType::RayTrace(RayTraceConfig::default()),
            )?
        } else {
            self.analyze_forward(
                rays.clone(),
                *center_thickness,
                &index_model.value.clone(),
                &eff_iso,
                &AnalyzerType::RayTrace(RayTraceConfig::default()),
            )?
        };
        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.to_string(), output);
        Ok(out_light_rays)
    }
}
