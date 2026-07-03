use crate::{
    analyzers::{AnalyzerType, RayTraceConfig, raytrace::AnalysisRayTrace},
    core_optics::NodeAttrExt,
    error::{OpmResult, OpossumError},
    light::{LightData, LightResult},
};

use super::BeamSplitter;

impl AnalysisRayTrace for BeamSplitter {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (input_port1, input_port2) = if self.inverted() {
            ("out1_trans1_refl2", "out2_trans2_refl1")
        } else {
            ("input_1", "input_2")
        };
        let in1 = incoming_data.get(input_port1);
        let in2 = incoming_data.get(input_port2);
        let (out1_data, out2_data) =
            self.analyze_raytrace(in1, in2, &AnalyzerType::RayTrace(config.clone()))?;
        if let Some(out1_data) = out1_data
            && let Some(out2_data) = out2_data
        {
            let (target1, target2) = if self.inverted() {
                ("input_1", "input_2")
            } else {
                ("out1_trans1_refl2", "out2_trans2_refl1")
            };
            Ok(LightResult::from([
                (target1.into(), out1_data),
                (target2.into(), out2_data),
            ]))
        } else {
            Ok(LightResult::default())
        }
    }
    fn calc_node_positions(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let (input_port1, _input_port2) = if self.inverted() {
            ("out1_trans1_refl2", "out2_trans2_refl1")
        } else {
            ("input_1", "input_2")
        };
        //todo: generally bullshit
        let refraction_intended = true;
        let in1 = incoming_data.get(input_port1);
        // todo: do this also for in2 and check for position inconsistencies....
        let out_rays = if let Some(input_1) = in1 {
            match input_1 {
                LightData::Geometric(r) => {
                    let mut rays = r.clone();
                    if let Some(surf) = self.get_optic_surface_mut(input_port1) {
                        rays.refract_on_surface(
                            surf,
                            None,
                            refraction_intended,
                            config.missed_surface_strategy(),
                        )?;
                    } else {
                        return Err(OpossumError::OpticPort(
                            "input optic surface not found".into(),
                        ));
                    }
                    rays
                }
                _ => {
                    return Err(OpossumError::Analysis(
                        "expected Rays value at `input_1` port".into(),
                    ));
                }
            }
        } else {
            return Err(OpossumError::Analysis(
                "could not calc optical axis for beam splitter".into(),
            ));
        };
        let (target1, target2) = if self.inverted() {
            ("input_1", "input_2")
        } else {
            ("out1_trans1_refl2", "out2_trans2_refl1")
        };
        let light_result = LightResult::from([
            (target1.into(), LightData::Geometric(out_rays.clone())),
            (target2.into(), LightData::Geometric(out_rays)),
        ]);
        Ok(light_result)
    }
}

#[cfg(test)]
mod test {
    use approx::assert_abs_diff_eq;

    use crate::{
        analyzers::{RayTraceConfig, raytrace::AnalysisRayTrace},
        core_optics::OpticNode,
        error::OpmResult,
        joule,
        light::{LightData, LightResult, Ray, Rays},
        millimeter, nanometer,
        nodes::{
            BeamSplitter, NodeGroup, SourcePort, SplittingConfigBuilder,
            round_collimated_ray_builder,
        },
        prelude::{AnalyzerType, OpmDocument},
        utils::geom_transformation::Isometry,
    };

    #[test]
    fn analyze_empty() -> OpmResult<()> {
        let mut node = BeamSplitter::default();
        let input = LightResult::default();
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_one_input() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        node.set_isometry(Isometry::identity())?;
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., 0.),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        input.insert("input_1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        let result = output.clone().get("out1_trans1_refl2").unwrap().clone();
        let energy = if let LightData::Geometric(r) = result {
            r.total_energy().get::<uom::si::energy::joule>()
        } else {
            0.0
        };
        assert_eq!(energy, 0.6);
        let result = output.clone().get("out2_trans2_refl1").unwrap().clone();
        let energy = if let LightData::Geometric(r) = result {
            r.total_energy().get::<uom::si::energy::joule>()
        } else {
            0.0
        };
        assert_eq!(energy, 0.4);
        Ok(())
    }
    #[test]
    fn analyze_two_input() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        node.set_isometry(Isometry::identity())?;
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        input.insert("input_1".into(), LightData::Geometric(rays));
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(0.5),
        )?);
        input.insert("input_2".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        let energy_output1 = if let LightData::Geometric(r) =
            output.clone().get("out1_trans1_refl2").unwrap().clone()
        {
            r.total_energy().get::<uom::si::energy::joule>()
        } else {
            0.0
        };
        assert_abs_diff_eq!(energy_output1, &0.8);
        let energy_output2 = if let LightData::Geometric(r) =
            output.clone().get("out2_trans2_refl1").unwrap().clone()
        {
            r.total_energy().get::<uom::si::energy::joule>()
        } else {
            0.0
        };
        assert_abs_diff_eq!(energy_output2, &0.7);
        Ok(())
    }
    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        node.set_isometry(Isometry::identity())?;
        node.set_inverted(true)?;
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        input.insert("out1_trans1_refl2".into(), LightData::Geometric(rays));
        let rays = Rays::from(Ray::new_collimated(
            millimeter!(0., 0., -10.),
            nanometer!(1053.0),
            joule!(0.5),
        )?);
        input.insert("out2_trans2_refl1".into(), LightData::Geometric(rays));
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        let energy_output1 =
            if let LightData::Geometric(r) = output.clone().get("input_1").unwrap().clone() {
                r.total_energy().get::<uom::si::energy::joule>()
            } else {
                0.0
            };
        assert_abs_diff_eq!(energy_output1, &0.8);
        let energy_output2 =
            if let LightData::Geometric(r) = output.clone().get("input_2").unwrap().clone() {
                r.total_energy().get::<uom::si::energy::joule>()
            } else {
                0.0
            };
        assert_abs_diff_eq!(energy_output2, &0.7);
        Ok(())
    }
    #[test]
    fn integration_not_connected_beam_splitter() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let src = scenery.add_node(SourcePort::default())?;
        scenery.add_node(BeamSplitter::default())?; // add unconnected beamsplitter
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?,
        );
        document.add_analyzer(AnalyzerType::RayTrace(config));
        document.analyze()?;
        Ok(())
    }
}
