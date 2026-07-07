use crate::{
    analyzers::ghostfocus::AnalysisGhostFocus,
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    light::{LightRays, Rays},
    nodes::SourcePort,
    prelude::{GhostFocusConfig, PortType},
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
            let Some(bouncing_rays) = incoming_data.get("output_1") else {
                return Err(OpossumError::Analysis("no light at port".into()));
            };
            bouncing_rays.clone()
        } else if bounce_lvl == 0 {
            // First pass: generate initial rays from RayDataBuilder in GhostFocusConfig
            let mut rays = config
                .get_source(&self.node_attr().uuid())
                .ok_or_else(|| {
                    OpossumError::Analysis(format!("No source data found in analyzer for {self}"))
                })?
                .clone()
                .build()?;

            let iso = self.effective_surface_iso("output_1")?;
            rays = rays.transformed_by_iso(&iso);

            // Evaluate and apply the aperture configuration on the output port
            match self.ports().aperture(&PortType::Output, "output_1") {
                Some(aperture) => {
                    rays.apodize(aperture, &iso)?;
                }
                _ => {
                    return Err(OpossumError::OpticPort("output aperture not found".into()));
                }
            }

            vec![rays]
        } else {
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
        distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
        joule,
        light::lightdata::ray_data_builder::RayDataBuilder,
        millimeter, nanometer,
        prelude::{Aperture, ApertureType, CollimatedSrc, RayDataSource},
    };

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
    fn analyze_ghostfocus_with_aperture_clipping() -> OpmResult<()> {
        let mut node = SourcePort::default();

        // 1. Setup a restrictive rectangular aperture on the output port
        let aperture = Aperture::new_rectangle(
            millimeter!(5.),
            millimeter!(5.),
            ApertureType::Hole,
            None,
            None,
        )?;
        node.ports_mut()
            .set_aperture(&PortType::Output, "output_1", &aperture)?;

        // 2. Create a hexapolar ray distribution wide enough to guarantee clipping (10mm radius)
        let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(10.), 3)?.into(),
            UniformDist::new(joule!(1.))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        ));

        let ray_data_builder = RayDataBuilder::from(ray_data_source);
        let mut ghost_config = GhostFocusConfig::default();
        ghost_config.map_source(node.node_attr().uuid(), ray_data_builder.clone());

        // 3. Run the GhostFocus analysis for the initial pass (bounce_lvl = 0)
        let mut ray_collection = Vec::new();
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            LightRays::default(),
            &ghost_config,
            &mut ray_collection,
            0,
        )?;

        // 4. Extract the resulting rays from the LightRays output mapping
        let rays_vec = output.get("output_1").unwrap();
        let rays_after = &rays_vec[0];

        let rays_before = ray_data_builder.build()?;

        // 5. Verification: Ensure the aperture actually clipped (reduced) the number of rays
        assert!(
            rays_after.nr_of_rays(true) < rays_before.nr_of_rays(true),
            "Rays were not clipped by the aperture during GhostFocus analysis!"
        );

        Ok(())
    }
}
