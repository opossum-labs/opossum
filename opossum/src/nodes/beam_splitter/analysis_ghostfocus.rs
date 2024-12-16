use super::BeamSplitter;
use crate::{
    analyzers::{ghostfocus::AnalysisGhostFocus, AnalyzerType, GhostFocusConfig},
    error::{OpmResult, OpossumError},
    light_result::LightRays,
    lightdata::LightData,
    optic_node::OpticNode,
    optic_ports::PortType,
    rays::Rays,
};

impl AnalysisGhostFocus for BeamSplitter {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let in1_port = &self.ports().names(&PortType::Input)[0];
        let in2_port = &self.ports().names(&PortType::Input)[1];
        let out1_port = &self.ports().names(&PortType::Output)[0];
        let out2_port = &self.ports().names(&PortType::Output)[1];

        let mut rays1_bundle = incoming_data
            .get(in1_port)
            .map_or_else(Vec::<Rays>::new, std::clone::Clone::clone);

        let mut rays2_bundle = incoming_data
            .get(in2_port)
            .map_or_else(Vec::<Rays>::new, std::clone::Clone::clone);

        let mut light_data_out1: Vec<Rays> = vec![];
        let mut light_data_out2: Vec<Rays> = vec![];
        for rays in &mut rays1_bundle {
            let (out1_data, out2_data) = self.analyze_raytrace(
                Some(&LightData::Geometric(rays.clone())),
                None,
                &AnalyzerType::GhostFocus(config.clone()),
            )?;
            if let Some(LightData::Geometric(rays)) = out1_data {
                light_data_out1.push(rays.clone());
            }
            if let Some(LightData::Geometric(rays)) = out2_data {
                light_data_out2.push(rays.clone());
            }
        }
        for rays in &mut rays2_bundle {
            let (out1_data, out2_data) = self.analyze_raytrace(
                None,
                Some(&LightData::Geometric(rays.clone())),
                &AnalyzerType::GhostFocus(config.clone()),
            )?;
            if let Some(LightData::Geometric(rays)) = out1_data {
                light_data_out1.push(rays.clone());
            }
            if let Some(LightData::Geometric(rays)) = out2_data {
                light_data_out2.push(rays.clone());
            }
        }
        let Some(surf) = self.get_optic_surface_mut(in1_port) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{in1_port}\" of node: \"{}\"",
                self.node_attr().name()
            )));
        };
        for rays in &mut rays1_bundle {
            surf.evaluate_fluence_of_ray_bundle(rays, config.fluence_estimator())?;
        }
        let Some(surf) = self.get_optic_surface_mut(in2_port) else {
            return Err(OpossumError::Analysis(format!(
                "Cannot find surface: \"{in2_port}\" of node: \"{}\"",
                self.node_attr().name()
            )));
        };
        for rays in &mut rays2_bundle {
            surf.evaluate_fluence_of_ray_bundle(rays, config.fluence_estimator())?;
        }
        let mut out_light_rays = LightRays::default();
        out_light_rays.insert(out1_port.to_string(), light_data_out1);
        out_light_rays.insert(out2_port.to_string(), light_data_out2);
        Ok(out_light_rays)
    }
}
