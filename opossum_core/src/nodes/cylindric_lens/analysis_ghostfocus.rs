use super::CylindricLens;
use crate::{
    analyzers::{GhostFocusConfig, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, PortType},
    error::OpmResult,
    light::{LightRays, Rays},
};

impl AnalysisGhostFocus for CylindricLens {
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
        // 1. Durch die Eintrittsfläche propagieren
        self.pass_through_surface_generic(
            in_port,
            Some(refri), // Als Option übergeben
            &mut rays_bundle,
            config, // config ist die PropagationStrategy
            self.inverted(),
            refraction_intended,
        )?;

        // 2. Durch die Austrittsfläche propagieren
        self.pass_through_surface_generic(
            out_port,
            Some(self.ambient_idx()), // Als Option übergeben
            &mut rays_bundle,
            config, // config ist die PropagationStrategy
            self.inverted(),
            refraction_intended,
        )?;

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out_port.clone(), rays_bundle);
        Ok(out_light_rays)
    }
}
