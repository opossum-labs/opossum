use nalgebra::Point3;
use num::Zero;
use uom::si::angle::Angle;

use super::Wedge;
use crate::{
    analyzers::{raytrace::AnalysisRayTrace, Analyzable, AnalyzerType, RayTraceConfig},
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    lightdata::LightData,
    optic_node::OpticNode,
    optic_ports::PortType,
    utils::geom_transformation::Isometry,
};

impl AnalysisRayTrace for Wedge {
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

        let (eff_iso, refri, center_thickness, wedge) =
            self.get_node_attributes_ray_trace(&self.node_attr)?;
        let thickness_iso = Isometry::new_along_z(center_thickness)?;
        let wedge_iso = Isometry::new(
            Point3::origin(),
            Point3::new(wedge, Angle::zero(), Angle::zero()),
        )?;

        if self.inverted() {
            self.set_surface_iso(out_port, &eff_iso)?;
            self.set_surface_iso(in_port, &eff_iso.append(&thickness_iso).append(&wedge_iso))?;
        } else {
            self.set_surface_iso(in_port, &eff_iso)?;
            self.set_surface_iso(out_port, &eff_iso.append(&thickness_iso).append(&wedge_iso))?;
        };

        let mut rays_bundle = vec![rays];
        let refraction_intended = true;
        self.pass_through_surface(
            in_port,
            &refri,
            &mut rays_bundle,
            &AnalyzerType::RayTrace(config.clone()),
            self.inverted(),
            refraction_intended,
        )?;
        self.pass_through_surface(
            out_port,
            &self.ambient_idx(),
            &mut rays_bundle,
            &AnalyzerType::RayTrace(config.clone()),
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
