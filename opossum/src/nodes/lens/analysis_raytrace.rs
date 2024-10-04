use super::Lens;
use crate::{
    analyzers::{raytrace::AnalysisRayTrace, AnalyzerType, RayTraceConfig},
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    lightdata::LightData,
    optic_node::OpticNode,
    properties::Proptype,
};

impl AnalysisRayTrace for Lens {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (in_port, out_port) = if self.inverted() {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        let LightData::Geometric(rays) = data.clone() else {
            return Err(OpossumError::Analysis(
                "expected ray data at input port".into(),
            ));
        };
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
                rays,
                *center_thickness,
                &index_model.value.clone(),
                &eff_iso,
                &AnalyzerType::RayTrace(config.clone()),
            )?
        } else {
            self.analyze_forward(
                rays,
                *center_thickness,
                &index_model.value.clone(),
                &eff_iso,
                &AnalyzerType::RayTrace(config.clone()),
            )?
        };
        let light_result = LightResult::from([(out_port.into(), LightData::Geometric(output))]);
        Ok(light_result)
    }
}
