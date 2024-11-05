use nalgebra::Point3;
use num::Zero;
use uom::si::f64::Angle;

use super::Wedge;
use crate::{
    analyzers::{
        ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace, Analyzable, AnalyzerType,
        GhostFocusConfig,
    },
    error::OpmResult,
    light_result::LightRays,
    optic_node::OpticNode,
    optic_ports::PortType,
    rays::Rays,
    utils::geom_transformation::Isometry,
};

impl AnalysisGhostFocus for Wedge {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];

        let mut rays_bundle = incoming_data
            .get(in_port)
            .map_or_else(Vec::<Rays>::new, std::clone::Clone::clone);

        let (eff_iso, refri, center_thickness, wedge) =
            self.get_node_attributes_ray_trace(&self.node_attr)?;
        let thickness_iso: Isometry = Isometry::new_along_z(center_thickness)?;
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

        let refraction_intended = true;
        self.pass_through_surface(
            in_port,
            &refri,
            &mut rays_bundle,
            &AnalyzerType::GhostFocus(config.clone()),
            self.inverted(),
            refraction_intended,
        )?;
        self.pass_through_surface(
            out_port,
            &self.ambient_idx(),
            &mut rays_bundle,
            &AnalyzerType::GhostFocus(config.clone()),
            self.inverted(),
            refraction_intended,
        )?;

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.to_string(), rays_bundle);
        Ok(out_light_rays)
    }
}
