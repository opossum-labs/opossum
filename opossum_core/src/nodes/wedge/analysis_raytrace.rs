use super::Wedge;
use crate::{
    analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, PortType},
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult},
};

impl AnalysisRayTrace for Wedge {
    fn analyze(
        &mut self,
        mut incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];

        let Some(data) = incoming_data.remove(in_port) else {
            return Ok(LightResult::default());
        };
        let LightData::Geometric(rays) = data else {
            return Err(OpossumError::Analysis(
                "expected ray data at input port".into(),
            ));
        };

        let (refri, _, _) = self.get_node_attributes_ray_trace(&self.node_attr)?;

        let mut rays_bundle = vec![rays];
        let refraction_intended = true;

        // 1. Eintrittsfläche
        self.pass_through_surface_generic(
            in_port,
            Some(refri),
            &mut rays_bundle,
            config,
            self.inverted(),
            refraction_intended,
        )?;

        // 2. Austrittsfläche
        self.pass_through_surface_generic(
            out_port,
            Some(self.ambient_idx()),
            &mut rays_bundle,
            config,
            self.inverted(),
            refraction_intended,
        )?;

        let light_result = LightResult::from([(
            out_port.into(),
            LightData::Geometric(rays_bundle[0].clone()),
        )]);
        Ok(light_result)
    }
}
