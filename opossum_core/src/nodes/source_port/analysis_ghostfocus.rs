use crate::{
    analyzers::ghostfocus::AnalysisGhostFocus,
    core_optics::{NodeAttrExt, OpticNodeExt, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    light::{LightRays, Rays},
    nodes::SourcePort,
    prelude::GhostFocusConfig,
};

impl AnalysisGhostFocus for SourcePort {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let rays = if self.inverted() {
            // In backward passes, forward returning ghost rays so they are recorded in the accumulated rays.
            // If no rays bounced back to the source port, do not error - simply treat it as an empty bundle.
            incoming_data.get("output_1").cloned().unwrap_or_default()
        } else if bounce_lvl == 0 {
            // First pass: generate initial rays from RayDataBuilder in GhostFocusConfig
            let mut rays = config
                .get_source(&self.node_attr().uuid())
                .ok_or_else(|| {
                    OpossumError::Analysis(format!("No source data found in analyzer for {self}"))
                })?
                .clone()
                .build()?;

            // Set ambient refractive index for initial ghost focus rays
            rays.set_refractive_index(config.ambient_material().refractive_index_type())?;

            let iso = self.effective_surface_iso("output_1")?;
            vec![rays.transformed_by_iso(&iso)]
        } else {
            // Forward bounce > 0: source port does not generate new initial rays
            Vec::<Rays>::new()
        };

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert("output_1".into(), rays);
        Ok(out_light_rays)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{core_optics::OpticNode, joule, light::Ray, millimeter, nanometer};

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

    #[test]
    fn analyze_ghostfocus_inverted_without_incoming_light() -> OpmResult<()> {
        let mut node = SourcePort::default();
        node.set_inverted(true)?;

        // Must succeed without error when no light arrives at the inverted source port
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            LightRays::default(),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            1,
        )?;

        let rays = output.get("output_1").expect("port mapping exists");
        assert!(rays.is_empty());
        Ok(())
    }

    #[test]
    fn analyze_ghostfocus_inverted_with_returning_rays() -> OpmResult<()> {
        let mut node = SourcePort::default();
        node.set_inverted(true)?;

        let ray = Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), nanometer!(1064.0), joule!(1.0))?;
        let rays = Rays::from(ray);
        let input = LightRays::from([("output_1".into(), vec![rays.clone()])]);

        let output = AnalysisGhostFocus::analyze(
            &mut node,
            input,
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            1,
        )?;

        assert_eq!(output.get("output_1"), Some(&vec![rays]));
        Ok(())
    }
}
