use log::info;

use crate::{
    analyzers::raytrace::AnalysisRayTrace, error::{OpmResult, OpossumError}, joule, light_result::LightResult, lightdata::LightData, millimeter, nodes::SourcePort, prelude::{OpticNode, PortType, Proptype, RayTraceConfig}, ray::Ray, rays::Rays
};

impl AnalysisRayTrace for SourcePort {
    fn analyze(
        &mut self,
        _incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let mut rays = config
            .get_source(&self.node_attr().uuid())
            .ok_or_else(|| {
                OpossumError::Analysis(format!("No source data found in analyzer for {self}"))
            })?
            .clone()
            .build()?;
        if let Ok(Proptype::Isometry(Some(iso))) = self.node_attr.get_property("light data iso") {
            rays = rays.transformed_by_iso(iso);
        }
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
            incoming_data: LightResult,
            config: &RayTraceConfig,
        ) -> OpmResult<LightResult> {
        let outgoing_edges = AnalysisRayTrace::analyze(self, incoming_data, config)?;
        // generate a single beam (= optical axis) from source
        let mut new_outgoing_edges = LightResult::new();
        for outgoing_edge in &outgoing_edges {
            if let LightData::Geometric(rays) = outgoing_edge.1 {
                let mut axis_ray = if let Ok(Proptype::LengthOption(Some(alignment_wvl))) =
                    self.node_attr.get_property("alignment wavelength")
                {
                    Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), *alignment_wvl, joule!(1.0))
                } else {
                    info!(
                        "No alignment wavelength defined, using energy-weighted central wavelength for alignment"
                    );
                    rays.get_optical_axis_ray()
                }?;
                let iso = self.effective_surface_iso("input_1")?;
                axis_ray = axis_ray.transformed_ray(&iso);
                let mut new_rays = Rays::default();
                new_rays.add_ray(axis_ray);
                new_outgoing_edges.insert(outgoing_edge.0.clone(), LightData::Geometric(new_rays));
            } else {
                return Err(OpossumError::Analysis(
                    "did not receive LightData:Geometric for conversion into OpticalAxis data"
                        .into(),
                ));
            }
        }
        Ok(new_outgoing_edges)
    }
}
