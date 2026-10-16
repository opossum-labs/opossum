use log::info;

use crate::{
    analyzers::{propagation_strategy::PropagationStrategy, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, OpticNodeExt, node_attr::HasNodeAttr},
    error::{OpmResult, OpossumError},
    joule,
    light::{LightData, LightResult, Ray, Rays},
    millimeter,
    nodes::SourcePort,
    prelude::RayTraceConfig,
};

impl AnalysisRayTrace for SourcePort {
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        // If the source port is inverted it acts as sink and does not emit any rays (since then it has no outgoing ports).
        if self.inverted() {
            return Ok(LightResult::default());
        }
        let mut rays = config
            .get_source(&self.node_attr().uuid())
            .ok_or_else(|| {
                OpossumError::Analysis(format!("No source data found in analyzer for {self}"))
            })?
            .clone()
            .build()?;

        // Apply the ambient refractive index to all newly created rays
        rays.set_refractive_index(&config.ambient_refractive_index())?;

        if let Ok(iso) = self.effective_surface_iso("output_1") {
            rays = rays.transformed_by_iso(&iso);
            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
        }
        Ok(LightResult::from([(
            "output_1".into(),
            LightData::Geometric(rays),
        )]))
    }

    fn calc_node_positions(
        &mut self,
        _incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        // Retrieve the source builder to read the alignment wavelength
        let ray_data_builder = config.get_source(&self.node_attr().uuid()).ok_or_else(|| {
            OpossumError::Analysis(format!("No source data found in analyzer for {self}"))
        })?;
        // The axis is a single ray: the source's rays are only built when no alignment wavelength
        // is given and their central wavelength has to stand in for it.
        let mut axis_ray = match ray_data_builder.alignment_wavelength() {
            Some(alignment_wvl) => {
                Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), alignment_wvl, joule!(1.0))?
            }
            None => {
                info!(
                    "No alignment wavelength defined, using energy-weighted central wavelength for alignment"
                );
                ray_data_builder.build()?.get_optical_axis_ray()?
            }
        };

        // Explicitly assign the ambient refractive index for the axis ray at its specific wavelength
        let ambient_n = config
            .ambient_refractive_index()
            .get_refractive_index(axis_ray.wavelength())?;
        axis_ray.set_refractive_index(ambient_n)?;

        let iso = self.effective_surface_iso("output_1")?;
        axis_ray = axis_ray.transformed_ray(&iso);
        let rays = Rays::from(axis_ray);
        let mut outgoing_edges = LightResult::new();
        outgoing_edges.insert("output_1".into(), LightData::Geometric(rays));
        Ok(outgoing_edges)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
        light::lightdata::ray_data_builder::RayDataBuilder,
        nanometer,
        prelude::{CollimatedSrc, ImageSrc, RayDataSource},
    };
    use approx::assert_relative_eq;
    use uom::si::{f64::Length, length::nanometer};

    /// A collimated source of a few rays, all at `wavelength`.
    fn collimated_at(wavelength: Length) -> OpmResult<RayDataBuilder> {
        Ok(RayDataBuilder::from(RayDataSource::Collimated(
            CollimatedSrc::new(
                Hexapolar::new(millimeter!(5.), 1)?.into(),
                UniformDist::new(joule!(1.))?.into(),
                LaserLines::new(vec![(wavelength, 1.0)])?.into(),
            ),
        )))
    }

    /// A ray-trace configuration feeding `node` the light `builder` describes.
    fn feeding(node: &SourcePort, builder: RayDataBuilder) -> RayTraceConfig {
        let mut config = RayTraceConfig::default();
        config.map_source(node.node_attr().uuid(), builder);
        config
    }

    /// The optical axis a positioning run sends out of `node`: a single ray.
    fn axis_ray_of(node: &mut SourcePort, config: &RayTraceConfig) -> OpmResult<Ray> {
        let output = AnalysisRayTrace::calc_node_positions(node, LightResult::default(), config)?;
        let Some(LightData::Geometric(rays)) = output.get("output_1") else {
            return Err(OpossumError::Other("no axis left the source".into()));
        };
        assert_eq!(rays.nr_of_rays(true), 1, "the axis is a single ray");
        rays.iter()
            .next()
            .cloned()
            .ok_or_else(|| OpossumError::Other("no axis ray".into()))
    }

    /// The axis leaves a source at its alignment wavelength, whatever its rays shine at.
    #[test]
    fn the_axis_leaves_at_the_alignment_wavelength() -> OpmResult<()> {
        let mut node = SourcePort::default();
        let mut builder = collimated_at(nanometer!(1000.0))?;
        builder.set_alignment_wavelength(Some(nanometer!(500.0)));
        let config = feeding(&node, builder);
        let axis = axis_ray_of(&mut node, &config)?;
        assert_relative_eq!(axis.wavelength().get::<nanometer>(), 500.0, epsilon = 1e-9);
        Ok(())
    }

    /// Without an alignment wavelength the axis takes the central wavelength of the source's rays.
    #[test]
    fn without_an_alignment_wavelength_the_axis_takes_that_of_the_rays() -> OpmResult<()> {
        let mut node = SourcePort::default();
        let config = feeding(&node, collimated_at(nanometer!(1000.0))?);
        let axis = axis_ray_of(&mut node, &config)?;
        assert_relative_eq!(axis.wavelength().get::<nanometer>(), 1000.0, epsilon = 1e-9);
        Ok(())
    }

    /// The axis carries the ambient medium's refractive index, with an alignment wavelength or
    /// without.
    #[test]
    fn the_axis_carries_the_ambient_refractive_index() -> OpmResult<()> {
        for alignment in [Some(nanometer!(500.0)), None] {
            let mut node = SourcePort::default();
            let mut builder = collimated_at(nanometer!(1000.0))?;
            builder.set_alignment_wavelength(alignment);
            let mut config = feeding(&node, builder);
            config.set_ambient_material(crate::material::Material::new_draft(
                "ambient",
                None,
                None,
                crate::refractive_index::RefrIndexConst::new(1.45)?.into(),
            ));
            let axis = axis_ray_of(&mut node, &config)?;
            assert_relative_eq!(axis.refractive_index(), 1.45, epsilon = 1e-12);
        }
        Ok(())
    }

    /// The axis is one ray, so with an alignment wavelength the source's own rays are never built:
    /// an image source whose file is missing still sends out its axis.
    #[test]
    fn an_alignment_wavelength_spares_building_the_rays() -> OpmResult<()> {
        let mut node = SourcePort::default();
        let mut builder = RayDataBuilder::from(RayDataSource::Image(ImageSrc::default()));
        builder.set_alignment_wavelength(Some(nanometer!(500.0)));
        let config = feeding(&node, builder);
        let axis = axis_ray_of(&mut node, &config)?;
        assert_relative_eq!(axis.wavelength().get::<nanometer>(), 500.0, epsilon = 1e-9);
        Ok(())
    }

    #[test]
    fn analyze_raytrace_no_source_definition() {
        let mut node = SourcePort::default();
        let output_error = AnalysisRayTrace::analyze(
            &mut node,
            LightResult::default(),
            &RayTraceConfig::default(),
        )
        .unwrap_err();
        assert!(
            output_error
                .to_string()
                .contains("No source data found in analyzer for")
        );
    }

    #[test]
    fn analyze_raytrace_ok() -> OpmResult<()> {
        let mut node = SourcePort::default();

        let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(10.), 1)?.into(),
            UniformDist::new(joule!(1.))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        ));

        // Wrap the data source inside the new RayDataBuilder
        let ray_data_builder = RayDataBuilder::from(ray_data_source);

        let mut ray_tracing_config = RayTraceConfig::default();
        ray_tracing_config.map_source(node.node_attr().uuid(), ray_data_builder.clone());

        let output =
            AnalysisRayTrace::analyze(&mut node, LightResult::default(), &ray_tracing_config)?;

        let LightData::Geometric(rays) = output.get("output_1").unwrap().clone() else {
            panic!("Expected LightData::Geometric");
        };

        let rays_from_ray_data_builder = ray_data_builder.build()?;
        assert_eq!(
            rays.nr_of_rays(true),
            rays_from_ray_data_builder.nr_of_rays(true)
        );
        assert_eq!(
            rays.total_energy(),
            rays_from_ray_data_builder.total_energy()
        );
        Ok(())
    }
    #[test]
    fn analyze_raytrace_ambient_medium() -> OpmResult<()> {
        let mut node = SourcePort::default();

        let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(5.), 1)?.into(),
            UniformDist::new(joule!(1.))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        ));
        let ray_data_builder = RayDataBuilder::from(ray_data_source);

        let mut config = RayTraceConfig::default();
        let ambient_index = 1.45;
        let ambient_mat = crate::material::Material::new_draft(
            "ambient",
            None,
            None,
            crate::refractive_index::RefrIndexConst::new(ambient_index)?.into(),
        );
        config.set_ambient_material(ambient_mat);
        config.map_source(node.node_attr().uuid(), ray_data_builder);

        let output = AnalysisRayTrace::analyze(&mut node, LightResult::default(), &config)?;
        let LightData::Geometric(rays) = output.get("output_1").unwrap().clone() else {
            panic!("Expected LightData::Geometric");
        };

        for ray in &rays {
            approx::assert_relative_eq!(ray.refractive_index(), ambient_index, epsilon = 1e-12);
        }
        Ok(())
    }
}
