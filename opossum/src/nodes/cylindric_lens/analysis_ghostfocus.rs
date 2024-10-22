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

use super::CylindricLens;

impl AnalysisGhostFocus for CylindricLens {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let (eff_iso, refri, center_thickness, _) =
            self.get_node_attributes_ray_trace(&self.node_attr)?;
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
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

        let Some(incoming_rays) = incoming_data.get(in_port) else {
            return Ok(LightRays::default());
        };
        let mut rays_bundle = incoming_rays.clone();

        self.enter_through_surface(
            &mut rays_bundle,
            &AnalyzerType::GhostFocus(config.clone()),
            &refri,
            self.inverted(),
            in_port,
        )?;
        self.exit_through_surface(
            &mut rays_bundle,
            &AnalyzerType::GhostFocus(config.clone()),
            &self.ambient_idx(),
            self.inverted(),
            out_port,
        )?;

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.to_string(), rays_bundle);
        Ok(out_light_rays)
    }
}
