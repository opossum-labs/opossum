use super::Lens;
use crate::{
    analyzers::{raytrace::AnalysisRayTrace, Analyzable, AnalyzerType, RayTraceConfig},
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    lightdata::LightData,
    optic_node::OpticNode,
    optic_ports::PortType,
    utils::geom_transformation::Isometry,
};

impl AnalysisRayTrace for Lens {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];

        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        let LightData::Geometric(rays) = data.clone() else {
            return Err(OpossumError::Analysis(
                "expected ray data at input port".into(),
            ));
        };

        let (eff_iso, refri, center_thickness, _) =
            self.get_node_attributes_ray_trace(&self.node_attr)?;
        let thickness_iso = Isometry::new_along_z(center_thickness)?;

        if self.inverted() {
            self.set_surface_iso_and_coating(out_port, &eff_iso, &PortType::Input)?;
            self.set_surface_iso_and_coating(
                in_port,
                &eff_iso.append(&thickness_iso),
                &PortType::Output,
            )?;
        } else {
            self.set_surface_iso_and_coating(in_port, &eff_iso, &PortType::Input)?;
            self.set_surface_iso_and_coating(
                out_port,
                &eff_iso.append(&thickness_iso),
                &PortType::Output,
            )?;
        };

        let mut rays_bundle = vec![rays];
        self.enter_through_surface(
            &mut rays_bundle,
            &AnalyzerType::RayTrace(config.clone()),
            &refri,
            self.inverted(),
            in_port,
        )?;
        self.exit_through_surface(
            &mut rays_bundle,
            &AnalyzerType::RayTrace(config.clone()),
            &self.ambient_idx(),
            self.inverted(),
            out_port,
        )?;

        let light_result = LightResult::from([(
            out_port.into(),
            LightData::Geometric(rays_bundle[0].clone()),
        )]);
        Ok(light_result)
    }
}
