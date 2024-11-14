use super::Lens;
use crate::{
    analyzers::{
        ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace, AnalyzerType, GhostFocusConfig,
    },
    error::OpmResult,
    light_result::LightRays,
    optic_node::OpticNode,
    optic_ports::PortType,
    rays::Rays,
};

impl AnalysisGhostFocus for Lens {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let (refri, _, _) = self.get_node_attributes_ray_trace(&self.node_attr)?;
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let mut rays_bundle = incoming_data
            .get(in_port)
            .map_or_else(Vec::<Rays>::new, std::clone::Clone::clone);

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
