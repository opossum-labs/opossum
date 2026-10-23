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
        let mut rays = ray_data_builder.build()?;

        // Ensure rays have the ambient refractive index set
        rays.set_refractive_index(config.ambient_material().refractive_index_type())?;

        let mut axis_ray = ray_data_builder.alignment_wavelength().map_or_else(|| {
                     info!(
                         "No alignment wavelength defined, using energy-weighted central wavelength for alignment"
                     );
                     rays.get_optical_axis_ray()
                 }, |alignment_wvl| Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), alignment_wvl, joule!(1.0)))?;

        // Explicitly assign the ambient refractive index for the axis ray at its specific wavelength
        let ambient_n = config
            .ambient_refractive_index()
            .get_refractive_index(axis_ray.wavelength())?;
        axis_ray.set_refractive_index(ambient_n)?;

        let iso = self.effective_surface_iso("output_1")?;
        axis_ray = axis_ray.transformed_ray(&iso);
        // The axis carries the up direction that places every node along it.
        axis_ray.set_up_direction(axis_ray.define_up_direction());
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
        prelude::{CollimatedSrc, RayDataSource},
    };

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
