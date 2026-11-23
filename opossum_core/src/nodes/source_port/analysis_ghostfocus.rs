use crate::{
    analyzers::ghostfocus::AnalysisGhostFocus,
    core_optics::{NodeAttrExt, node_attr::HasNodeAttr},
    error::OpmResult,
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
            match self.light_or_warn(config.get_source(&self.node_attr().uuid())) {
                Some(ray_data_builder) => vec![self.sent_out_rays(ray_data_builder, config)?],
                None => Vec::new(),
            }
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
    use crate::{
        core_optics::OpticNode,
        joule,
        light::{Ray, lightdata::ray_data_builder::RayDataBuilder},
        millimeter, nanometer,
        nodes::round_collimated_ray_builder,
        prelude::RayDataSource,
    };

    /// A source spans ray tubes where its ghost focus analysis needs them, and only there. Raw
    /// rays span them too.
    #[test]
    fn a_source_spans_ray_tubes_where_its_analysis_needs_them() -> OpmResult<()> {
        let raw = RayDataBuilder::from(RayDataSource::Raw(
            round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?.build()?,
        ));
        for spanned in [true, false] {
            for builder in [
                round_collimated_ray_builder(millimeter!(1.0), joule!(1.0), 1)?,
                raw.clone(),
            ] {
                let mut node = SourcePort::default();
                let mut config = GhostFocusConfig::default();
                config.set_ray_tubes(spanned);
                config.map_source(node.node_attr().uuid(), builder);
                let output = AnalysisGhostFocus::analyze(
                    &mut node,
                    LightRays::default(),
                    &config,
                    &mut Vec::new(),
                    0,
                )?;
                let rays = output.get("output_1").expect("port mapping exists");
                assert_eq!(rays.len(), 1, "{spanned}");
                assert!(
                    rays.iter()
                        .flat_map(Rays::iter)
                        .all(|ray| ray.source().is_some() == spanned),
                    "{spanned}"
                );
            }
        }
        Ok(())
    }

    /// A source the analysis defines no light for sends out none, and says so.
    #[test]
    fn analyze_ghostfocus_no_source_definition() -> OpmResult<()> {
        let mut node = SourcePort::default();
        testing_logger::setup();
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            LightRays::default(),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            0,
        )?;
        assert!(output.values().all(Vec::is_empty), "{output:?}");
        crate::utils::test_helper::helper::assert_warned("has no light in this analysis");
        Ok(())
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
