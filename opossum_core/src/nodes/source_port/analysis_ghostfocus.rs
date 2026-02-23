use crate::{
    analyzers::{ghostfocus::AnalysisGhostFocus, raytrace::MissedSurfaceStrategy},
    error::{OpmResult, OpossumError},
    light_result::LightRays,
    nodes::SourcePort,
    prelude::{GhostFocusConfig, OpticNode},
    rays::Rays,
};

impl AnalysisGhostFocus for SourcePort {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let mut rays = if self.inverted() {
            let Some(bouncing_rays) = incoming_data.get("output_1") else {
                return Err(OpossumError::Analysis("no light at port".into()));
            };
            bouncing_rays.clone()
            // first pass: generate rays from RayDataBuilder in GhostFocusConfig
        } else if bounce_lvl == 0 {
            let mut rays = config
                .get_source(&self.node_attr.uuid())
                .ok_or_else(|| {
                    OpossumError::Analysis(format!("No source data found in analyzer for {self}"))
                })?
                .clone()
                .build()?;

            // Apply node's surface isometry (the local space transformation)
            let iso = self.effective_surface_iso("output_1")?;
            rays = rays.transformed_by_iso(&iso);
            vec![rays]
        } else {
            Vec::<Rays>::new()
        };

        if let Some(surf) = self.get_optic_surface_mut("input_1") {
            let refraction_intended = true;
            for r in &mut rays {
                r.refract_on_surface(
                    surf,
                    None,
                    refraction_intended,
                    &MissedSurfaceStrategy::Ignore,
                )?;
                surf.evaluate_fluence_of_ray_bundle(r, config.fluence_estimator())?;
            }
        } else {
            return Err(OpossumError::Analysis("no surface found. Aborting".into()));
        }

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert("output_1".into(), rays);
        Ok(out_light_rays)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn analyze_ghostfocus_no_source_definition() {
        let mut node = SourcePort::default();
        let output_error = AnalysisGhostFocus::analyze(
            &mut node,
            LightRays::default(),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            0,
        )
        .unwrap_err();
        assert!(
            output_error
                .to_string()
                .contains("No source data found in analyzer for")
        );
    }
}
