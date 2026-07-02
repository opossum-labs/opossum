use log::info;

use crate::{
    analyzers::raytrace::AnalysisRayTrace,
    core_optics::node_attr::HasNodeAttr,
    error::{OpmResult, OpossumError},
    joule,
    light::{LightData, LightResult, Ray, Rays},
    millimeter,
    nodes::SourcePort,
    prelude::{OpticNode, PortType, RayTraceConfig},
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

        if let Ok(iso) = self.effective_surface_iso("input_1") {
            rays = rays.transformed_by_iso(&iso);
            // consider aperture only if not inverted (there is only an output port)
            if !self.inverted() {
                match self.ports().aperture(&PortType::Output, "output_1") {
                    Some(aperture) => {
                        rays.apodize(aperture, &iso)?;
                        rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                    }
                    _ => {
                        return Err(OpossumError::OpticPort("output aperture not found".into()));
                    }
                }
            }
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
        let rays = ray_data_builder.build()?;
        let mut axis_ray = ray_data_builder.alignment_wavelength().map_or_else(|| {
                     info!(
                         "No alignment wavelength defined, using energy-weighted central wavelength for alignment"
                     );
                     rays.get_optical_axis_ray()
                 }, |alignment_wvl| Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), alignment_wvl, joule!(1.0)))?;
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
}
