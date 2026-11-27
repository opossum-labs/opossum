use log::warn;

use crate::{
    analyzers::{propagation_strategy::PropagationStrategy, raytrace::AnalysisRayTrace},
    core_optics::{NodeAttrExt, OpticNodeExt, node_attr::HasNodeAttr},
    error::OpmResult,
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
        if self.effective_node_iso().is_none() {
            warn!("{self} is not placed, so it sends out no light");
            return Ok(LightResult::default());
        }
        let Some(ray_data_builder) =
            self.light_or_warn(config.get_source(&self.node_attr().uuid()))
        else {
            return Ok(LightResult::default());
        };
        let mut rays = self.sent_out_rays(ray_data_builder, config)?;
        rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
        Ok(LightResult::from([(
            "output_1".into(),
            LightData::Geometric(rays),
        )]))
    }

    /// Sends out the optical axis: one ray from the center of the output face, along its normal,
    /// at the [`alignment wavelength`](SourcePort::alignment_wavelength) of this source port.
    ///
    /// The light an analysis defines for the source plays no part: the setup stands where it
    /// stands, whatever is sent through it.
    fn calc_node_positions(
        &mut self,
        _incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let mut axis_ray = Ray::new_collimated(
            millimeter!(0.0, 0.0, 0.0),
            self.alignment_wavelength()?,
            joule!(1.0),
        )?;

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
        core_optics::{OpticNode, node_attr::NodePositioning},
        degree,
        distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
        error::OpossumError,
        light::lightdata::ray_data_builder::RayDataBuilder,
        nanometer,
        nodes::ALIGNMENT_WAVELENGTH,
        prelude::{CollimatedSrc, ImageSrc, RayDataSource},
        utils::{geom_transformation::Isometry, test_helper::helper::assert_warned},
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

    /// The axis leaves a source port at its own alignment wavelength, whatever the light the
    /// analysis sends in through it shines at.
    #[test]
    fn the_axis_leaves_at_the_alignment_wavelength_of_the_port() -> OpmResult<()> {
        let mut node = SourcePort::default();
        node.set_property(ALIGNMENT_WAVELENGTH, nanometer!(500.0).into())?;
        let config = feeding(&node, collimated_at(nanometer!(1000.0))?);
        let axis = axis_ray_of(&mut node, &config)?;
        assert_relative_eq!(axis.wavelength().get::<nanometer>(), 500.0, epsilon = 1e-9);
        Ok(())
    }

    /// A new source port places the setup at the central wavelength of the default source.
    #[test]
    fn a_new_port_is_aligned_at_the_default_wavelength() -> OpmResult<()> {
        let mut node = SourcePort::default();
        let axis = axis_ray_of(&mut node, &RayTraceConfig::default())?;
        assert_relative_eq!(
            axis.wavelength().get::<nanometer>(),
            SourcePort::default_alignment_wavelength()?.get::<nanometer>(),
            epsilon = 1e-9
        );
        Ok(())
    }

    /// Light the analysis places off the port - shifted and tilted - does not move the axis: it
    /// still leaves the center of the output face along its normal.
    #[test]
    fn the_light_of_the_analysis_does_not_move_the_axis() -> OpmResult<()> {
        let mut node = SourcePort::default();
        let mut builder = collimated_at(nanometer!(1000.0))?;
        builder.set_isometry(Some(Isometry::new(
            millimeter!(5.0, 0.0, 0.0),
            degree!(0.0, 10.0, 0.0),
        )?));
        let config = feeding(&node, builder);
        let axis = axis_ray_of(&mut node, &config)?;
        assert_relative_eq!(axis.position().x.value, 0.0, epsilon = 1e-12);
        assert_relative_eq!(axis.direction().x, 0.0, epsilon = 1e-12);
        assert_relative_eq!(axis.direction().y, 0.0, epsilon = 1e-12);
        Ok(())
    }

    /// The axis carries the ambient medium's refractive index.
    #[test]
    fn the_axis_carries_the_ambient_refractive_index() -> OpmResult<()> {
        let mut node = SourcePort::default();
        let mut config = RayTraceConfig::default();
        config.set_ambient_material(crate::material::Material::new_draft(
            "ambient",
            None,
            None,
            crate::refractive_index::RefrIndexConst::new(1.45)?.into(),
        ));
        let axis = axis_ray_of(&mut node, &config)?;
        assert_relative_eq!(axis.refractive_index(), 1.45, epsilon = 1e-12);
        Ok(())
    }

    /// The axis is one ray, so the source's own rays are never built for it: an image source
    /// whose file is missing still sends out its axis.
    #[test]
    fn the_axis_never_builds_the_rays_of_the_source() -> OpmResult<()> {
        let mut node = SourcePort::default();
        let builder = RayDataBuilder::from(RayDataSource::Image(ImageSrc::default()));
        let config = feeding(&node, builder);
        axis_ray_of(&mut node, &config)?;
        Ok(())
    }

    /// A source that is not placed sends out no light, and says so, rather than shining from the
    /// origin.
    #[test]
    fn a_source_without_a_placement_sends_out_no_light() -> OpmResult<()> {
        let mut node = SourcePort::default();
        node.set_positioning(NodePositioning::Automatic(None))?;
        let config = feeding(&node, collimated_at(nanometer!(1000.0))?);
        testing_logger::setup();
        let output = AnalysisRayTrace::analyze(&mut node, LightResult::default(), &config)?;
        assert!(output.is_empty(), "{output:?}");
        assert_warned("is not placed");
        Ok(())
    }

    /// A source the analysis defines no light for sends out no rays, and says so.
    #[test]
    fn analyze_raytrace_no_source_definition() -> OpmResult<()> {
        let mut node = SourcePort::default();
        let config = RayTraceConfig::default();
        testing_logger::setup();
        assert!(AnalysisRayTrace::analyze(&mut node, LightResult::default(), &config)?.is_empty());
        assert_warned("has no light in this analysis");
        Ok(())
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
